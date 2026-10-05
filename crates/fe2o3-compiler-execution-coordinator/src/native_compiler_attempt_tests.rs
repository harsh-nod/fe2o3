//! Mechanical controls only; genuine execution belongs to the installed matrix.
use super::*;

#[test]
fn launch_consumes_actual_helper_and_preserves_original_work_lifetime() {
    let _: for<'work> unsafe fn(
        Helper,
        &Receiver,
        Credentials,
        Instant,
        &mut Cleanup,
        &mut Budget<'work>,
    ) -> AttemptResult<(Attempt<'work>, usize)> = launch;
}

#[test]
fn envelope_accounts_for_inventory_trace_stage_and_all_three_unreleased_gate_status_owners() {
    assert_eq!(
        Attempt::ENVELOPE
            + size_of::<Trace<'static>>()
            + size_of::<Executables>()
            + size_of::<Stage>()
            + 3 * size_of::<OwnedFd>(),
        size_of::<Attempt<'static>>(),
    );
    assert!(Attempt::ENVELOPE >= size_of::<usize>());
}

#[test]
fn equal_contents_cannot_replace_an_original_staged_object() {
    let original = tempfile::tempfile().unwrap();
    let other = tempfile::tempfile().unwrap();
    let duplicate = original.try_clone().unwrap();
    same_object(original.as_fd(), &duplicate).unwrap();
    assert!(matches!(
        same_object(original.as_fd(), &other),
        Err(Failure::Invalid(_))
    ));
    assert_eq!(
        original.metadata().unwrap().len(),
        other.metadata().unwrap().len()
    );
}

#[test]
fn final_validation_requires_both_typed_child_modes() {
    for (checkpoints, confinement) in [(false, false), (false, true), (true, false)] {
        assert!(require_stage_modes(checkpoints, confinement).is_err());
    }
    require_stage_modes(true, true).unwrap();
}

#[test]
fn runtime_step_quote_composes_original_owner_helper_and_controller() {
    let quote = Attempt::runtime_step_quota().unwrap();
    let trace = crate::native_v3::NativeAttempt::<Helper>::runtime_backing_quota().unwrap();
    let helper = Helper::checkpoint_access_quota().unwrap();
    assert_eq!(
        quote.work(),
        LOCAL_WORK + trace.work() + helper.work() + Controller::step_work().unwrap()
    );
    assert_eq!(
        quote.scratch(),
        FRAME + trace.scratch() + helper.scratch() + Controller::STEP_SCRATCH
    );
}

#[test]
fn runtime_arm_and_cancellation_quotes_include_the_outer_attempt() {
    let arm = Attempt::arm_runtime_quota().unwrap();
    let inner = Trace::runtime_takeover_quota().unwrap();
    assert_eq!(arm.work(), LOCAL_WORK + inner.work());
    assert_eq!(arm.scratch(), FRAME + inner.scratch());
    let cancel = Attempt::cancellation_quota().unwrap();
    let issued = crate::native_v3::NativeAttempt::<Helper>::runtime_cancellation_quota().unwrap();
    assert_eq!(cancel.work(), LOCAL_WORK + issued.work());
    assert_eq!(cancel.scratch(), FRAME + issued.scratch());
}

#[test]
fn runtime_poll_confirmation_and_capture_have_closed_finite_quotes() {
    let poll = Attempt::runtime_poll_quota().unwrap();
    let original = Trace::gated_operation_quota().unwrap();
    assert_eq!(
        (poll.work(), poll.scratch()),
        (original.work(), original.scratch())
    );
    let confirm = Attempt::runtime_confirmation_quota().unwrap();
    let inner = Trace::runtime_confirmation_quota().unwrap();
    assert_eq!(confirm.work(), LOCAL_WORK + inner.work());
    assert_eq!(confirm.scratch(), FRAME + inner.scratch());
    let validation = super::super::quota::refusal().unwrap();
    let interrupt = super::super::quota::runtime_interrupt().unwrap();
    assert_eq!(interrupt.work(), validation.work() + original.work());
    let capture = super::super::quota::runtime_capture().unwrap();
    let trace = Trace::runtime_backing_quota().unwrap();
    assert_eq!(
        capture.work(),
        validation.work() + trace.work() + Controller::INITIAL_IMAGE_WORK
    );
    assert_eq!(
        capture.scratch(),
        validation.scratch() + trace.scratch() + Controller::INITIAL_IMAGE_SCRATCH
    );
}

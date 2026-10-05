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

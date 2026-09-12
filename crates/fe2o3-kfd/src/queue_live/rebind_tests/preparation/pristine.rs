//! The continuation and rebind data come from one real fixture preparation/abort.

use super::*;

#[path = "pristine/facade.rs"]
mod facade;

pub(super) fn fixture(
    next: u64,
) -> (
    LoanFixture,
    Vec<Gfx942FixedDispatchDataV1>,
    PristineDispatchContinuationV1,
    u64,
) {
    let (mut f, data) = super::fixture();
    let before = f.memory.observation();
    let identities = fixed_dispatch_storage_identities(&data);
    let loan = f.memory.primary_loan(&mut f.foundation).unwrap();
    let mut preparation =
        FixedDispatchPreparationCustodyV1::new([packet(0), packet(1), packet(2)], data);
    prepare_public_fixed_dispatch_resources_after_detach_in_place(
        &mut f.memory,
        &programs(),
        &mut preparation,
        next - 1,
    )
    .unwrap();
    let owner = preparation.take_completed().unwrap();
    assert_eq!(owner.primary_fixture_next_generation_v1(), next);
    let occurrence = owner.primary_fixture_recipe_occurrence_v1();
    let buffers = owner.prepare_pristine_abort_v1().unwrap();
    let mut abort = owner.begin_pristine_abort_v1(buffers);
    abort.release_controls(&mut f.memory).unwrap();
    let (continuation, data, returned_identities) = abort.into_detached();
    assert_eq!(continuation.next_generation_for_test(), next);
    assert_eq!(returned_identities, identities);
    assert_eq!(fixed_dispatch_storage_identities(&data), identities);
    f.memory.primary_reclaim(&mut f.foundation, loan).unwrap();
    f.memory.assert_data_unchanged(&before);
    assert_eq!(&f.memory.observation().calls[5..], &[4, 4, 4]);
    (f, data, continuation, occurrence)
}

pub(in crate::queue) fn pristine_settlement_regression_v1() {
    for (closing, validation) in [
        (Closing::None, Validation::None),
        (Closing::PostError, Validation::None),
        (Closing::PostPanic, Validation::None),
        (Closing::None, Validation::Error),
        (Closing::None, Validation::Panic),
    ] {
        exercise_source(
            Source::Pristine(8),
            Opening::None,
            None,
            closing,
            validation,
            false,
        );
    }
}

pub(in crate::queue) fn pristine_constructor_regression_v1() {
    exercise_source(
        Source::Pristine(8),
        Opening::None,
        Some((PreparationStageV1::Plan, false)),
        Closing::None,
        Validation::None,
        false,
    );
}

#[test]
fn pristine_rebind_preflight_retains_exact_inputs_without_entered_poison_policy() {
    preflight_source(true);
}

#[test]
fn pristine_rebind_preserves_real_continuation_generation_and_fresh_occurrence() {
    for next in [1, 7, 8, u64::MAX - 1] {
        exercise_source(
            Source::Pristine(next),
            Opening::None,
            None,
            Closing::None,
            Validation::None,
            false,
        );
    }
}

#[test]
fn pristine_rebind_real_loan_operation_retake_matrix_preserves_custody() {
    for operation in [
        None,
        Some((PreparationStageV1::CodeResolve(0), false)),
        Some((PreparationStageV1::CodeResolve(0), true)),
    ] {
        for closing in [
            Closing::None,
            Closing::PreError,
            Closing::PrePanic,
            Closing::PostError,
            Closing::PostPanic,
            Closing::Regress,
        ] {
            exercise_source(
                Source::Pristine(7),
                Opening::None,
                operation,
                closing,
                Validation::None,
                false,
            );
        }
    }
}

#[test]
fn pristine_rebind_opening_failure_retains_unconsumed_continuation() {
    for opening in [Opening::Error, Opening::Panic, Opening::Exhausted] {
        exercise_source(
            Source::Pristine(7),
            opening,
            None,
            Closing::None,
            Validation::None,
            false,
        );
    }
}

#[test]
fn pristine_rebind_failed_complete_cannot_be_suppressed() {
    for validation in [Validation::None, Validation::Bypass] {
        exercise_source(
            Source::Pristine(7),
            Opening::None,
            Some((PreparationStageV1::Complete, false)),
            Closing::None,
            validation,
            true,
        );
    }
}

#[test]
fn pristine_rebind_corrupted_continuation_records_failed_generation() {
    // A real producer cannot issue MAX. This is defensive internal corruption coverage.
    exercise_source(
        Source::PristineInvalid,
        Opening::None,
        None,
        Closing::None,
        Validation::None,
        false,
    );
}

#[test]
fn pristine_rebind_all_preparation_stages_retain_original_prefixes() {
    let mut stages = vec![
        PreparationStageV1::Generation,
        PreparationStageV1::Plan,
        PreparationStageV1::Capacity,
        PreparationStageV1::DataRetention,
        PreparationStageV1::KernargAllocate,
        PreparationStageV1::KernargMaterialize,
        PreparationStageV1::KernargMap,
        PreparationStageV1::KernargRetain,
        PreparationStageV1::Commit,
        PreparationStageV1::Complete,
    ];
    for i in 0..3 {
        stages.extend([
            PreparationStageV1::CodeAllocate(i),
            PreparationStageV1::CodeMaterialize(i),
            PreparationStageV1::CodeSeal(i),
            PreparationStageV1::CodeMap(i),
            PreparationStageV1::CodeRetain(i),
            PreparationStageV1::CodeResolve(i),
            PreparationStageV1::PacketResolve(i),
        ]);
    }
    assert_eq!(stages.len(), 31);
    for stage in stages {
        for panic in [false, true] {
            exercise_source(
                Source::Pristine(7),
                Opening::None,
                Some((stage, panic)),
                Closing::None,
                Validation::None,
                false,
            );
        }
    }
}

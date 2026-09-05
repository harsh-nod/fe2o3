use alloc::format;

use super::r36_fused_completion_poll_recycle::*;

fn binding() -> R36CompletionBindingV1 {
    R36CompletionBindingV1 {
        queue_id: 11,
        queue_generation: 13,
        attachment_generation: 17,
        dispatch_generation: 19,
        completion_batch_id: 23,
        signal_slot: 0,
        signal_generation: 29,
        next_signal_generation: 30,
    }
}

fn observations(
    poll: R36PollObservationV1,
    recycle: R36RecycleObservationV1,
) -> R36CompletionObservationsV1 {
    R36CompletionObservationsV1 {
        poll,
        split_recycle_opening_currentness_succeeded: true,
        completion_midpoint: 101,
        recycle,
    }
}

fn model() -> R36CompletionPollRecycleModelV1 {
    R36CompletionPollRecycleModelV1::new_model_only(binding()).unwrap()
}

#[test]
fn binding_requires_exact_nonzero_incarnations_and_monotonic_signal_generation() {
    assert!(binding().is_valid());
    for invalid in [
        R36CompletionBindingV1 {
            queue_id: 0,
            ..binding()
        },
        R36CompletionBindingV1 {
            queue_generation: 0,
            ..binding()
        },
        R36CompletionBindingV1 {
            attachment_generation: 0,
            ..binding()
        },
        R36CompletionBindingV1 {
            dispatch_generation: 0,
            ..binding()
        },
        R36CompletionBindingV1 {
            completion_batch_id: 0,
            ..binding()
        },
        R36CompletionBindingV1 {
            next_signal_generation: 31,
            ..binding()
        },
        R36CompletionBindingV1 {
            signal_generation: u64::MAX,
            next_signal_generation: 0,
            ..binding()
        },
    ] {
        assert_eq!(
            R36CompletionPollRecycleModelV1::new_model_only(invalid),
            Err(R36ModelErrorV1::InvalidBinding)
        );
    }
}

#[test]
fn pending_short_circuits_without_midpoint_reset_or_recycle() {
    let observations = observations(
        R36PollObservationV1::Pending,
        R36RecycleObservationV1::Recycled,
    );
    for state in [
        model().run_split_model_only(observations),
        model().run_fused_model_only(observations),
    ] {
        assert_eq!(state.outcome, R36OutcomeV1::Pending);
        assert_eq!(state.custody, R36CustodyV1::Published);
        assert_eq!(state.failure_route, None);
        assert_eq!(state.completion_midpoint, None);
        assert_eq!(state.midpoint_event_index, None);
        assert_eq!(state.signal_reset_event_index, None);
        assert_eq!(state.dispatch_recycle_event_index, None);
        assert_eq!(state.attachment_recycle_event_index, None);
        assert_eq!(state.currentness_check_count, 2);
        assert!(state.has_exactly_one_stage_authority());
    }
}

#[test]
fn ready_captures_midpoint_after_both_completions_and_before_reset() {
    let state = model().run_fused_model_only(observations(
        R36PollObservationV1::Ready,
        R36RecycleObservationV1::Recycled,
    ));
    assert_eq!(state.completion_midpoint, Some(101));
    assert!(
        state.dispatch_completion_event_index.unwrap()
            < state.allocation_completion_event_index.unwrap()
    );
    assert!(state.allocation_completion_event_index.unwrap() < state.midpoint_event_index.unwrap());
    assert!(state.midpoint_event_index.unwrap() < state.signal_reset_event_index.unwrap());
    assert!(state.successful_recycle_is_ordered());
}

#[test]
fn every_failure_has_exact_custody_and_poll_or_recycle_route() {
    for (poll, custody) in [
        (
            R36PollObservationV1::PublishedStateFailure,
            R36CustodyV1::Published,
        ),
        (
            R36PollObservationV1::DispatchGenerationFailure,
            R36CustodyV1::Published,
        ),
        (
            R36PollObservationV1::CompletionObservationFailure,
            R36CustodyV1::Published,
        ),
        (
            R36PollObservationV1::DispatchCompletionFailure,
            R36CustodyV1::Completed,
        ),
        (
            R36PollObservationV1::AllocationCompletionFailure,
            R36CustodyV1::Completed,
        ),
    ] {
        let state =
            model().run_fused_model_only(observations(poll, R36RecycleObservationV1::Recycled));
        assert_eq!(state.outcome, R36OutcomeV1::Terminal);
        assert_eq!(state.custody, custody);
        assert_eq!(state.failure_route, Some(R36FailureRouteV1::Poll));
        assert!(state.terminal_poisoned);
        assert!(state.has_exactly_one_stage_authority());
    }

    for (recycle, custody) in [
        (
            R36RecycleObservationV1::SignalGenerationFailure,
            R36CustodyV1::Completed,
        ),
        (
            R36RecycleObservationV1::SignalResetFailure,
            R36CustodyV1::Completed,
        ),
        (
            R36RecycleObservationV1::ClosingCurrentnessFailure,
            R36CustodyV1::Completed,
        ),
        (
            R36RecycleObservationV1::RecycleCurrentnessFailure,
            R36CustodyV1::Completed,
        ),
        (
            R36RecycleObservationV1::RecycleInfrastructureFailure,
            R36CustodyV1::Completed,
        ),
        (
            R36RecycleObservationV1::DispatchRecycleFailure,
            R36CustodyV1::Recycled,
        ),
    ] {
        let state =
            model().run_fused_model_only(observations(R36PollObservationV1::Ready, recycle));
        assert_eq!(state.outcome, R36OutcomeV1::Terminal);
        assert_eq!(state.custody, custody);
        assert_eq!(state.failure_route, Some(R36FailureRouteV1::Recycle));
        assert_eq!(state.completion_midpoint, Some(101));
        assert!(state.terminal_poisoned);
        assert!(state.has_exactly_one_stage_authority());
    }
}

#[test]
fn successful_fusion_preserves_projection_and_reduces_four_checks_to_three() {
    let observations = observations(
        R36PollObservationV1::Ready,
        R36RecycleObservationV1::Recycled,
    );
    let split = model().run_split_model_only(observations);
    let fused = model().run_fused_model_only(observations);
    assert!(split.same_projected_custody_and_ordering_semantics(&fused));
    assert_eq!(split.currentness_check_count, 4);
    assert_eq!(fused.currentness_check_count, 3);
    assert!(split.all_currentness_observations_succeeded);
    assert!(fused.all_currentness_observations_succeeded);
}

#[test]
fn removed_split_opening_failure_is_excluded_by_the_input_premise() {
    let mut observations = observations(
        R36PollObservationV1::Ready,
        R36RecycleObservationV1::Recycled,
    );
    observations.split_recycle_opening_currentness_succeeded = false;
    assert!(!observations.fusion_premise());
    let split = model().run_split_model_only(observations);
    let fused = model().run_fused_model_only(observations);
    assert_eq!(split.custody, R36CustodyV1::Completed);
    assert_eq!(split.failure_route, Some(R36FailureRouteV1::Recycle));
    assert_eq!(fused.custody, R36CustodyV1::Recycled);
    assert!(!split.same_projected_custody_and_ordering_semantics(&fused));
}

#[test]
fn premise_is_input_only_and_does_not_invoke_either_runner() {
    let source = include_str!("r36_fused_completion_poll_recycle.rs");
    let premise = source
        .split("pub const fn fusion_premise")
        .nth(1)
        .unwrap()
        .split("#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum R36OutcomeV1")
        .next()
        .unwrap();
    assert!(!premise.contains("run_split_model_only"));
    assert!(!premise.contains("run_fused_model_only"));
    assert!(!premise.contains("same_projected_custody_and_ordering_semantics"));
}

#[test]
fn stage_authority_carriers_are_private_and_move_only() {
    let source = include_str!("r36_fused_completion_poll_recycle.rs");
    for carrier in [
        "R36PublishedAuthorityV1",
        "R36CompletedAuthorityV1",
        "R36RecycledAuthorityV1",
    ] {
        assert!(source.contains(&format!("struct {carrier}")));
        assert!(!source.contains(&format!("pub struct {carrier}")));
        assert!(!source.contains(&format!("#[derive(Clone)]\nstruct {carrier}")));
        assert!(!source.contains(&format!("#[derive(Clone, Copy)]\nstruct {carrier}")));
        assert!(!source.contains(&format!("impl Clone for {carrier}")));
        assert!(!source.contains(&format!("impl Copy for {carrier}")));
    }
}

#[test]
fn exhaustive_finite_premise_implies_projected_custody_and_ordering_equivalence() {
    let polls = [
        R36PollObservationV1::PublishedStateFailure,
        R36PollObservationV1::DispatchGenerationFailure,
        R36PollObservationV1::CompletionObservationFailure,
        R36PollObservationV1::DispatchCompletionFailure,
        R36PollObservationV1::AllocationCompletionFailure,
        R36PollObservationV1::Pending,
        R36PollObservationV1::Ready,
    ];
    let recycles = [
        R36RecycleObservationV1::SignalGenerationFailure,
        R36RecycleObservationV1::SignalResetFailure,
        R36RecycleObservationV1::ClosingCurrentnessFailure,
        R36RecycleObservationV1::RecycleCurrentnessFailure,
        R36RecycleObservationV1::RecycleInfrastructureFailure,
        R36RecycleObservationV1::DispatchRecycleFailure,
        R36RecycleObservationV1::Recycled,
    ];
    let mut total = 0_u16;
    let mut admitted = 0_u16;
    for poll in polls {
        for split_recycle_opening_currentness_succeeded in [false, true] {
            for completion_midpoint in [101, 103] {
                for recycle in recycles {
                    total += 1;
                    let observations = R36CompletionObservationsV1 {
                        poll,
                        split_recycle_opening_currentness_succeeded,
                        completion_midpoint,
                        recycle,
                    };
                    let split = model().run_split_model_only(observations);
                    let fused = model().run_fused_model_only(observations);
                    assert!(split.has_exactly_one_stage_authority());
                    assert!(fused.has_exactly_one_stage_authority());
                    if !observations.fusion_premise() {
                        continue;
                    }
                    admitted += 1;
                    assert!(
                        split.same_projected_custody_and_ordering_semantics(&fused),
                        "premised input diverged: {observations:?}\nsplit={split:?}\nfused={fused:?}"
                    );
                }
            }
        }
    }
    assert_eq!(total, 196);
    assert_eq!(admitted, 182);
}

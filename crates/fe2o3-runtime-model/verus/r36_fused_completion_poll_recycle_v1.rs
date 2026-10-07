// Independent finite R36 proof model for fused completion poll and recycle.
// All observations are contracted mathematical inputs. This file proves no
// Rust-to-Verus, production-Rust, native, hardware, clock, completion-truth,
// progress, liveness, or performance refinement. The projection preserves
// abstract custody, failure routing, midpoint, authority cardinality, and
// logical ordering while excluding the currentness-check count. Successful
// split and fused check counts are proved separately as four and three.
use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct BindingV1 {
    pub queue_id: nat,
    pub queue_generation: nat,
    pub attachment_generation: nat,
    pub dispatch_generation: nat,
    pub completion_batch_id: nat,
    pub signal_slot: nat,
    pub signal_generation: nat,
    pub next_signal_generation: nat,
}

#[derive(PartialEq, Eq)]
pub enum PollObservationV1 {
    PublishedStateFailure,
    DispatchGenerationFailure,
    CompletionObservationFailure,
    DispatchCompletionFailure,
    AllocationCompletionFailure,
    Pending,
    Ready,
}

#[derive(PartialEq, Eq)]
pub enum RecycleObservationV1 {
    SignalGenerationFailure,
    SignalResetFailure,
    ClosingCurrentnessFailure,
    RecycleCurrentnessFailure,
    RecycleInfrastructureFailure,
    DispatchRecycleFailure,
    Recycled,
}

#[derive(PartialEq, Eq)]
pub struct ObservationsV1 {
    pub poll: PollObservationV1,
    pub split_recycle_opening_currentness_succeeded: bool,
    pub completion_midpoint: nat,
    pub recycle: RecycleObservationV1,
}

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 { Pending, Recycled, Terminal }

#[derive(PartialEq, Eq)]
pub enum CustodyV1 { Published, Completed, Recycled }

#[derive(PartialEq, Eq)]
pub enum FailureRouteV1 { Poll, Recycle }

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub binding: BindingV1,
    pub outcome: OutcomeV1,
    pub custody: CustodyV1,
    pub failure_route: Option<FailureRouteV1>,
    pub terminal_poisoned: bool,
    pub completion_midpoint: Option<nat>,
    pub poll_event: nat,
    pub dispatch_completion_event: Option<nat>,
    pub allocation_completion_event: Option<nat>,
    pub midpoint_event: Option<nat>,
    pub signal_reset_event: Option<nat>,
    pub closing_currentness_event: Option<nat>,
    pub dispatch_recycle_event: Option<nat>,
    pub attachment_recycle_event: Option<nat>,
    pub published_authorities: nat,
    pub completed_authorities: nat,
    pub recycled_authorities: nat,
    pub currentness_checks: nat,
    pub all_currentness_observations_succeeded: bool,
}

// These mathematical carriers are copyable. They establish no Rust ownership
// or borrow property; executable Rust has separate structural and compile-fail
// checks for its move-only carriers.
pub struct PublishedAuthorityV1 { pub binding: BindingV1 }
pub struct CompletedAuthorityV1 { pub binding: BindingV1 }
pub struct RecycledAuthorityV1 { pub binding: BindingV1 }

pub open spec fn valid_binding_v1(binding: BindingV1) -> bool {
    &&& binding.queue_id > 0
    &&& binding.queue_generation > 0
    &&& binding.attachment_generation > 0
    &&& binding.dispatch_generation > 0
    &&& binding.completion_batch_id > 0
    &&& binding.next_signal_generation == binding.signal_generation + 1
}

/// Input-only premise. It invokes neither execution relation and compares no
/// result state.
pub open spec fn fusion_premise_v1(observations: ObservationsV1) -> bool {
    observations.poll != PollObservationV1::Ready
        || observations.split_recycle_opening_currentness_succeeded
}

pub open spec fn poll_is_failure_v1(poll: PollObservationV1) -> bool {
    poll != PollObservationV1::Pending && poll != PollObservationV1::Ready
}

pub open spec fn poll_failure_custody_v1(poll: PollObservationV1) -> CustodyV1 {
    if poll == PollObservationV1::PublishedStateFailure
        || poll == PollObservationV1::DispatchGenerationFailure
        || poll == PollObservationV1::CompletionObservationFailure {
        CustodyV1::Published
    } else {
        CustodyV1::Completed
    }
}

pub open spec fn recycle_is_failure_v1(recycle: RecycleObservationV1) -> bool {
    recycle != RecycleObservationV1::Recycled
}

pub open spec fn recycle_failure_custody_v1(recycle: RecycleObservationV1) -> CustodyV1 {
    if recycle == RecycleObservationV1::DispatchRecycleFailure {
        CustodyV1::Recycled
    } else {
        CustodyV1::Completed
    }
}

pub open spec fn reset_attempted_v1(recycle: RecycleObservationV1) -> bool {
    recycle != RecycleObservationV1::SignalGenerationFailure
        && recycle != RecycleObservationV1::RecycleCurrentnessFailure
        && recycle != RecycleObservationV1::RecycleInfrastructureFailure
}

pub open spec fn closing_attempted_v1(recycle: RecycleObservationV1) -> bool {
    recycle == RecycleObservationV1::ClosingCurrentnessFailure
        || recycle == RecycleObservationV1::DispatchRecycleFailure
        || recycle == RecycleObservationV1::Recycled
}

pub open spec fn dispatch_recycle_attempted_v1(recycle: RecycleObservationV1) -> bool {
    recycle == RecycleObservationV1::DispatchRecycleFailure
        || recycle == RecycleObservationV1::Recycled
}

pub open spec fn currentness_failure_v1(recycle: RecycleObservationV1) -> bool {
    recycle == RecycleObservationV1::ClosingCurrentnessFailure
        || recycle == RecycleObservationV1::RecycleCurrentnessFailure
}

pub open spec fn initial_state_v1(binding: BindingV1) -> StateV1 {
    StateV1 {
        binding,
        outcome: OutcomeV1::Terminal,
        custody: CustodyV1::Published,
        failure_route: None,
        terminal_poisoned: false,
        completion_midpoint: None,
        poll_event: 1,
        dispatch_completion_event: None,
        allocation_completion_event: None,
        midpoint_event: None,
        signal_reset_event: None,
        closing_currentness_event: None,
        dispatch_recycle_event: None,
        attachment_recycle_event: None,
        published_authorities: 1,
        completed_authorities: 0,
        recycled_authorities: 0,
        currentness_checks: 0,
        all_currentness_observations_succeeded: true,
    }
}

pub open spec fn with_custody_v1(state: StateV1, custody: CustodyV1) -> StateV1 {
    StateV1 {
        custody,
        published_authorities: if custody == CustodyV1::Published { 1 } else { 0 },
        completed_authorities: if custody == CustodyV1::Completed { 1 } else { 0 },
        recycled_authorities: if custody == CustodyV1::Recycled { 1 } else { 0 },
        ..state
    }
}

pub open spec fn poll_failure_state_v1(
    binding: BindingV1,
    poll: PollObservationV1,
) -> StateV1 {
    let state = initial_state_v1(binding);
    let state = StateV1 {
        outcome: OutcomeV1::Terminal,
        failure_route: Some(FailureRouteV1::Poll),
        terminal_poisoned: true,
        currentness_checks:
            if poll == PollObservationV1::CompletionObservationFailure { 1 }
            else if poll == PollObservationV1::DispatchCompletionFailure
                || poll == PollObservationV1::AllocationCompletionFailure { 2 }
            else { 0 },
        all_currentness_observations_succeeded:
            poll != PollObservationV1::CompletionObservationFailure,
        dispatch_completion_event:
            if poll == PollObservationV1::AllocationCompletionFailure { Some(2) }
            else { None },
        ..state
    };
    with_custody_v1(state, poll_failure_custody_v1(poll))
}

pub open spec fn pending_state_v1(binding: BindingV1) -> StateV1 {
    StateV1 {
        outcome: OutcomeV1::Pending,
        currentness_checks: 2,
        ..initial_state_v1(binding)
    }
}

pub open spec fn ready_midpoint_state_v1(
    binding: BindingV1,
    observations: ObservationsV1,
    fused: bool,
) -> StateV1 {
    let state = StateV1 {
        custody: CustodyV1::Completed,
        completion_midpoint: Some(observations.completion_midpoint),
        dispatch_completion_event: Some(2),
        allocation_completion_event: Some(3),
        midpoint_event: Some(4),
        published_authorities: 0,
        completed_authorities: 1,
        currentness_checks: if fused { 2 } else { 3 },
        ..initial_state_v1(binding)
    };
    state
}

pub open spec fn split_opening_failure_state_v1(state: StateV1) -> StateV1 {
    StateV1 {
        outcome: OutcomeV1::Terminal,
        failure_route: Some(FailureRouteV1::Recycle),
        terminal_poisoned: true,
        all_currentness_observations_succeeded: false,
        ..state
    }
}

pub open spec fn recycle_failure_state_v1(
    state: StateV1,
    recycle: RecycleObservationV1,
) -> StateV1 {
    let state = StateV1 {
        outcome: OutcomeV1::Terminal,
        failure_route: Some(FailureRouteV1::Recycle),
        terminal_poisoned: true,
        signal_reset_event: if reset_attempted_v1(recycle) { Some(5) } else { None },
        closing_currentness_event: if closing_attempted_v1(recycle) { Some(6) } else { None },
        dispatch_recycle_event:
            if dispatch_recycle_attempted_v1(recycle) { Some(7) } else { None },
        currentness_checks:
            state.currentness_checks + if closing_attempted_v1(recycle) { 1nat } else { 0nat },
        all_currentness_observations_succeeded: !currentness_failure_v1(recycle),
        ..state
    };
    with_custody_v1(state, recycle_failure_custody_v1(recycle))
}

pub open spec fn recycled_state_v1(state: StateV1) -> StateV1 {
    with_custody_v1(StateV1 {
        outcome: OutcomeV1::Recycled,
        signal_reset_event: Some(5),
        closing_currentness_event: Some(6),
        dispatch_recycle_event: Some(7),
        attachment_recycle_event: Some(8),
        currentness_checks: state.currentness_checks + 1,
        ..state
    }, CustodyV1::Recycled)
}

pub open spec fn execute_v1(
    binding: BindingV1,
    observations: ObservationsV1,
    fused: bool,
) -> StateV1 {
    if poll_is_failure_v1(observations.poll) {
        poll_failure_state_v1(binding, observations.poll)
    } else if observations.poll == PollObservationV1::Pending {
        pending_state_v1(binding)
    } else {
        let state = ready_midpoint_state_v1(binding, observations, fused);
        if !fused && !observations.split_recycle_opening_currentness_succeeded {
            split_opening_failure_state_v1(state)
        } else if recycle_is_failure_v1(observations.recycle) {
            recycle_failure_state_v1(state, observations.recycle)
        } else {
            recycled_state_v1(state)
        }
    }
}

pub open spec fn projected_custody_and_ordering_equal_v1(
    left: StateV1,
    right: StateV1,
) -> bool {
    &&& left.binding == right.binding
    &&& left.outcome == right.outcome
    &&& left.custody == right.custody
    &&& left.failure_route == right.failure_route
    &&& left.terminal_poisoned == right.terminal_poisoned
    &&& left.completion_midpoint == right.completion_midpoint
    &&& left.poll_event == right.poll_event
    &&& left.dispatch_completion_event == right.dispatch_completion_event
    &&& left.allocation_completion_event == right.allocation_completion_event
    &&& left.midpoint_event == right.midpoint_event
    &&& left.signal_reset_event == right.signal_reset_event
    &&& left.closing_currentness_event == right.closing_currentness_event
    &&& left.dispatch_recycle_event == right.dispatch_recycle_event
    &&& left.attachment_recycle_event == right.attachment_recycle_event
    &&& left.published_authorities == right.published_authorities
    &&& left.completed_authorities == right.completed_authorities
    &&& left.recycled_authorities == right.recycled_authorities
    &&& left.all_currentness_observations_succeeded
        == right.all_currentness_observations_succeeded
}

pub open spec fn exactly_one_stage_authority_v1(state: StateV1) -> bool {
    state.published_authorities + state.completed_authorities + state.recycled_authorities == 1
}

pub proof fn valid_binding_has_strict_signal_successor_v1(binding: BindingV1)
    requires valid_binding_v1(binding),
    ensures binding.next_signal_generation > binding.signal_generation,
{
    reveal(valid_binding_v1);
}

pub proof fn non_ready_paths_admit_any_split_opening_observation_v1(observations: ObservationsV1)
    requires observations.poll != PollObservationV1::Ready,
    ensures fusion_premise_v1(observations),
{
    reveal(fusion_premise_v1);
}

pub proof fn pending_short_circuits_before_midpoint_and_recycle_v1(
    binding: BindingV1,
    observations: ObservationsV1,
    fused: bool,
)
    requires observations.poll == PollObservationV1::Pending,
    ensures
        execute_v1(binding, observations, fused).outcome == OutcomeV1::Pending,
        execute_v1(binding, observations, fused).custody == CustodyV1::Published,
        execute_v1(binding, observations, fused).completion_midpoint.is_none(),
        execute_v1(binding, observations, fused).signal_reset_event.is_none(),
        execute_v1(binding, observations, fused).dispatch_recycle_event.is_none(),
{
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(pending_state_v1);
    reveal(initial_state_v1);
}

pub proof fn ready_captures_midpoint_after_completion_before_reset_v1(
    binding: BindingV1,
    observations: ObservationsV1,
    fused: bool,
)
    requires
        observations.poll == PollObservationV1::Ready,
        fused || observations.split_recycle_opening_currentness_succeeded,
    ensures
        execute_v1(binding, observations, fused).completion_midpoint
            == Some(observations.completion_midpoint),
        execute_v1(binding, observations, fused).dispatch_completion_event == Some(2),
        execute_v1(binding, observations, fused).allocation_completion_event == Some(3),
        execute_v1(binding, observations, fused).midpoint_event == Some(4),
        execute_v1(binding, observations, fused).signal_reset_event.is_some()
            ==> 4 < execute_v1(binding, observations, fused).signal_reset_event.unwrap(),
{
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(ready_midpoint_state_v1);
    reveal(recycle_is_failure_v1);
    reveal(recycle_failure_state_v1);
    reveal(recycled_state_v1);
    reveal(reset_attempted_v1);
    reveal(closing_attempted_v1);
    reveal(dispatch_recycle_attempted_v1);
    reveal(with_custody_v1);
}

pub proof fn poll_failures_route_as_poll_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires poll_is_failure_v1(observations.poll),
    ensures
        execute_v1(binding, observations, true).outcome == OutcomeV1::Terminal,
        execute_v1(binding, observations, true).failure_route == Some(FailureRouteV1::Poll),
        execute_v1(binding, observations, true).terminal_poisoned,
{
    reveal(execute_v1);
    reveal(poll_failure_state_v1);
    reveal(initial_state_v1);
    reveal(with_custody_v1);
}

pub proof fn early_poll_failures_retain_published_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires
        observations.poll == PollObservationV1::PublishedStateFailure
            || observations.poll == PollObservationV1::DispatchGenerationFailure
            || observations.poll == PollObservationV1::CompletionObservationFailure,
    ensures execute_v1(binding, observations, true).custody == CustodyV1::Published,
{
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(poll_failure_state_v1);
    reveal(poll_failure_custody_v1);
    reveal(with_custody_v1);
}

pub proof fn late_poll_failures_retain_completed_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires
        observations.poll == PollObservationV1::DispatchCompletionFailure
            || observations.poll == PollObservationV1::AllocationCompletionFailure,
    ensures execute_v1(binding, observations, true).custody == CustodyV1::Completed,
{
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(poll_failure_state_v1);
    reveal(poll_failure_custody_v1);
    reveal(with_custody_v1);
}

pub proof fn recycle_failures_route_as_recycle_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires
        observations.poll == PollObservationV1::Ready,
        recycle_is_failure_v1(observations.recycle),
    ensures
        execute_v1(binding, observations, true).outcome == OutcomeV1::Terminal,
        execute_v1(binding, observations, true).failure_route == Some(FailureRouteV1::Recycle),
        execute_v1(binding, observations, true).terminal_poisoned,
{
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(ready_midpoint_state_v1);
    reveal(recycle_failure_state_v1);
    reveal(with_custody_v1);
}

pub proof fn pre_retirement_recycle_failures_retain_completed_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires
        observations.poll == PollObservationV1::Ready,
        recycle_is_failure_v1(observations.recycle),
        observations.recycle != RecycleObservationV1::DispatchRecycleFailure,
    ensures execute_v1(binding, observations, true).custody == CustodyV1::Completed,
{
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(ready_midpoint_state_v1);
    reveal(recycle_failure_state_v1);
    reveal(recycle_failure_custody_v1);
    reveal(with_custody_v1);
}

pub proof fn dispatch_recycle_failure_retains_recycled_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires
        observations.poll == PollObservationV1::Ready,
        observations.recycle == RecycleObservationV1::DispatchRecycleFailure,
    ensures execute_v1(binding, observations, true).custody == CustodyV1::Recycled,
{
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(recycle_is_failure_v1);
    reveal(ready_midpoint_state_v1);
    reveal(recycle_failure_state_v1);
    reveal(recycle_failure_custody_v1);
    reveal(with_custody_v1);
}

pub proof fn every_outcome_has_one_stage_authority_v1(
    binding: BindingV1,
    observations: ObservationsV1,
    fused: bool,
)
    ensures exactly_one_stage_authority_v1(execute_v1(binding, observations, fused)),
{
    reveal(exactly_one_stage_authority_v1);
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(poll_failure_state_v1);
    reveal(poll_failure_custody_v1);
    reveal(pending_state_v1);
    reveal(ready_midpoint_state_v1);
    reveal(recycle_is_failure_v1);
    reveal(recycle_failure_state_v1);
    reveal(recycle_failure_custody_v1);
    reveal(recycled_state_v1);
    reveal(initial_state_v1);
    reveal(with_custody_v1);
}

pub proof fn successful_fused_recycle_is_ordered_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires
        observations.poll == PollObservationV1::Ready,
        observations.recycle == RecycleObservationV1::Recycled,
    ensures
        execute_v1(binding, observations, true).midpoint_event.unwrap()
            < execute_v1(binding, observations, true).signal_reset_event.unwrap(),
        execute_v1(binding, observations, true).signal_reset_event.unwrap()
            < execute_v1(binding, observations, true).closing_currentness_event.unwrap(),
        execute_v1(binding, observations, true).closing_currentness_event.unwrap()
            < execute_v1(binding, observations, true).dispatch_recycle_event.unwrap(),
        execute_v1(binding, observations, true).dispatch_recycle_event.unwrap()
            < execute_v1(binding, observations, true).attachment_recycle_event.unwrap(),
{
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(recycle_is_failure_v1);
    reveal(ready_midpoint_state_v1);
    reveal(recycled_state_v1);
    reveal(with_custody_v1);
}

pub proof fn successful_fusion_reduces_four_checks_to_three_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires
        observations.poll == PollObservationV1::Ready,
        observations.split_recycle_opening_currentness_succeeded,
        observations.recycle == RecycleObservationV1::Recycled,
    ensures
        execute_v1(binding, observations, false).currentness_checks == 4,
        execute_v1(binding, observations, true).currentness_checks == 3,
        execute_v1(binding, observations, false).all_currentness_observations_succeeded,
        execute_v1(binding, observations, true).all_currentness_observations_succeeded,
{
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(recycle_is_failure_v1);
    reveal(ready_midpoint_state_v1);
    reveal(recycled_state_v1);
    reveal(with_custody_v1);
}

pub proof fn premised_projected_custody_and_ordering_equivalence_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires fusion_premise_v1(observations),
    ensures projected_custody_and_ordering_equal_v1(
        execute_v1(binding, observations, false),
        execute_v1(binding, observations, true)),
{
    reveal(fusion_premise_v1);
    reveal(projected_custody_and_ordering_equal_v1);
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(poll_failure_state_v1);
    reveal(poll_failure_custody_v1);
    reveal(pending_state_v1);
    reveal(ready_midpoint_state_v1);
    reveal(recycle_is_failure_v1);
    reveal(recycle_failure_state_v1);
    reveal(recycle_failure_custody_v1);
    reveal(reset_attempted_v1);
    reveal(closing_attempted_v1);
    reveal(dispatch_recycle_attempted_v1);
    reveal(currentness_failure_v1);
    reveal(recycled_state_v1);
    reveal(initial_state_v1);
    reveal(with_custody_v1);
}

pub proof fn failed_removed_opening_check_demonstrates_premise_boundary_v1(
    binding: BindingV1,
    observations: ObservationsV1,
)
    requires
        observations.poll == PollObservationV1::Ready,
        !observations.split_recycle_opening_currentness_succeeded,
        observations.recycle == RecycleObservationV1::Recycled,
    ensures
        !fusion_premise_v1(observations),
        execute_v1(binding, observations, false).custody == CustodyV1::Completed,
        execute_v1(binding, observations, true).custody == CustodyV1::Recycled,
{
    reveal(fusion_premise_v1);
    reveal(execute_v1);
    reveal(poll_is_failure_v1);
    reveal(recycle_is_failure_v1);
    reveal(ready_midpoint_state_v1);
    reveal(split_opening_failure_state_v1);
    reveal(recycled_state_v1);
    reveal(with_custody_v1);
}

} // verus!

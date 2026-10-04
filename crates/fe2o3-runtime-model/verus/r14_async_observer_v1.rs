use vstd::prelude::*;

verus! {

pub open spec fn max_async_waiters_v1() -> nat { 65_536 }

#[derive(PartialEq, Eq)]
pub enum RuntimeStatusV1 {
    Pending,
    Succeeded,
    Failed,
    QuiescentWithoutResult,
}

#[derive(PartialEq, Eq)]
pub enum ObserverOutcomeV1 {
    Waiting,
    Runtime(RuntimeStatusV1),
    RuntimeError,
    EngineStopped,
    Abandoned,
}

pub struct ObserverStateV1 {
    pub context_generation: nat,
    pub event_id: nat,
    pub registered: bool,
    pub outcome: ObserverOutcomeV1,
    pub submission_retained: bool,
    pub event_retained: bool,
    pub submission_cancelled: bool,
    pub submission_released: bool,
    pub event_released: bool,
}

pub open spec fn valid_event_identity_v1(state: ObserverStateV1) -> bool {
    state.context_generation > 0 && state.event_id > 0
}

pub open spec fn valid_pending_observer_v1(state: ObserverStateV1) -> bool {
    &&& valid_event_identity_v1(state)
    &&& state.registered
    &&& state.outcome == ObserverOutcomeV1::Waiting
    &&& state.submission_retained
    &&& state.event_retained
    &&& !state.submission_cancelled
    &&& !state.submission_released
    &&& !state.event_released
}

pub open spec fn registration_allowed_v1(
    waiter_count: nat,
    capacity: nat,
    duplicate: bool,
    context_generation: nat,
    event_id: nat,
) -> bool {
    &&& 0 < capacity <= max_async_waiters_v1()
    &&& waiter_count < capacity
    &&& !duplicate
    &&& context_generation > 0
    &&& event_id > 0
}

pub open spec fn register_count_v1(
    waiter_count: nat,
    capacity: nat,
    duplicate: bool,
    context_generation: nat,
    event_id: nat,
) -> nat {
    if registration_allowed_v1(
        waiter_count,
        capacity,
        duplicate,
        context_generation,
        event_id,
    ) {
        waiter_count + 1
    } else {
        waiter_count
    }
}

pub open spec fn observe_status_v1(
    state: ObserverStateV1,
    observed: RuntimeStatusV1,
) -> ObserverStateV1 {
    if observed == RuntimeStatusV1::Pending {
        state
    } else {
        ObserverStateV1 {
            registered: false,
            outcome: ObserverOutcomeV1::Runtime(observed),
            ..state
        }
    }
}

pub open spec fn observe_runtime_error_v1(state: ObserverStateV1) -> ObserverStateV1 {
    ObserverStateV1 {
        registered: false,
        outcome: ObserverOutcomeV1::RuntimeError,
        ..state
    }
}

pub open spec fn abandon_v1(state: ObserverStateV1) -> ObserverStateV1 {
    ObserverStateV1 {
        registered: false,
        outcome: ObserverOutcomeV1::Abandoned,
        ..state
    }
}

pub open spec fn stop_v1(state: ObserverStateV1) -> ObserverStateV1 {
    ObserverStateV1 {
        registered: false,
        outcome: ObserverOutcomeV1::EngineStopped,
        ..state
    }
}

pub open spec fn event_key_less_v1(
    first_context: nat,
    first_event: nat,
    second_context: nat,
    second_event: nat,
) -> bool {
    first_context < second_context
        || (first_context == second_context && first_event < second_event)
}

pub proof fn configuration_bound_v1(capacity: nat)
    requires 0 < capacity <= max_async_waiters_v1(),
    ensures capacity <= 65_536,
{
}

pub proof fn invalid_identity_registration_is_atomic_v1(
    waiter_count: nat,
    capacity: nat,
    duplicate: bool,
    context_generation: nat,
    event_id: nat,
)
    requires context_generation == 0 || event_id == 0,
    ensures register_count_v1(
        waiter_count,
        capacity,
        duplicate,
        context_generation,
        event_id,
    ) == waiter_count,
{
}

pub proof fn duplicate_registration_is_atomic_v1(
    waiter_count: nat,
    capacity: nat,
    context_generation: nat,
    event_id: nat,
)
    ensures register_count_v1(
        waiter_count,
        capacity,
        true,
        context_generation,
        event_id,
    ) == waiter_count,
{
}

pub proof fn capacity_registration_is_atomic_v1(
    waiter_count: nat,
    capacity: nat,
    duplicate: bool,
    context_generation: nat,
    event_id: nat,
)
    requires waiter_count >= capacity,
    ensures register_count_v1(
        waiter_count,
        capacity,
        duplicate,
        context_generation,
        event_id,
    ) == waiter_count,
{
}

pub proof fn pending_observation_preserves_waiter_v1(state: ObserverStateV1)
    requires valid_pending_observer_v1(state),
    ensures observe_status_v1(state, RuntimeStatusV1::Pending) == state,
{
}

pub proof fn terminal_observation_is_exact_v1(
    state: ObserverStateV1,
    observed: RuntimeStatusV1,
)
    requires
        valid_pending_observer_v1(state),
        observed != RuntimeStatusV1::Pending,
    ensures
        !observe_status_v1(state, observed).registered,
        observe_status_v1(state, observed).outcome == ObserverOutcomeV1::Runtime(observed),
{
}

pub proof fn runtime_error_observation_is_exact_v1(state: ObserverStateV1)
    requires valid_pending_observer_v1(state),
    ensures
        !observe_runtime_error_v1(state).registered,
        observe_runtime_error_v1(state).outcome == ObserverOutcomeV1::RuntimeError,
{
}

pub proof fn abandon_preserves_runtime_custody_v1(state: ObserverStateV1)
    requires valid_pending_observer_v1(state),
    ensures
        abandon_v1(state).submission_retained == state.submission_retained,
        abandon_v1(state).event_retained == state.event_retained,
        abandon_v1(state).submission_cancelled == state.submission_cancelled,
        abandon_v1(state).submission_released == state.submission_released,
        abandon_v1(state).event_released == state.event_released,
{
}

pub proof fn stop_preserves_runtime_custody_v1(state: ObserverStateV1)
    requires valid_pending_observer_v1(state),
    ensures
        stop_v1(state).submission_retained == state.submission_retained,
        stop_v1(state).event_retained == state.event_retained,
        stop_v1(state).submission_cancelled == state.submission_cancelled,
        stop_v1(state).submission_released == state.submission_released,
        stop_v1(state).event_released == state.event_released,
{
}

pub proof fn event_key_order_is_lexicographic_v1(
    first_context: nat,
    first_event: nat,
    second_context: nat,
    second_event: nat,
)
    requires
        first_context < second_context
            || (first_context == second_context && first_event < second_event),
    ensures event_key_less_v1(
        first_context,
        first_event,
        second_context,
        second_event,
    ),
{
}

}

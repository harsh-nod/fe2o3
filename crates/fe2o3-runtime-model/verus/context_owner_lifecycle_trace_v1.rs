// Trace premises record raw executions, not matched answers or reached invariants.
verus! {

struct LifecycleOriginV1 {
    actual_before: ConstructorObservationsV1,
    actual_after: ConstructorObservationsV1,
    model_before: logical::ConstructorObservationsV1,
    model_after: logical::ConstructorObservationsV1,
    context: u64,
    allocations: usize,
    writers: usize,
    reads: usize,
    actual_result: Result<ContextProducerReadJournalV1, ContextVersionJournalErrorV1>,
    model_result: Result<logical::ProducerReadContentsV1, logical::ConstructorErrorV1>,
}

enum LifecyclePairedEventV1 {
    Mutation { actual: LifecycleActualStepV1, model: logical::LifecycleStepV1 },
    Observe { query: LifecycleQueryV1, actual_answer: LifecycleAnswerV1, model_answer: LifecycleAnswerV1 },
}

spec fn lifecycle_origin_relation_v1(origin: LifecycleOriginV1) -> bool {
    &&& constructor_observations_match_v1(origin.actual_before, origin.model_before)
    &&& constructor_producer_relation_v1(origin.actual_before, origin.actual_after, origin.context,
        origin.allocations, origin.writers, origin.reads, origin.actual_result)
    &&& logical::constructor_producer_relation_v1(origin.model_before, origin.model_after, origin.context,
        origin.allocations, origin.writers, origin.reads, origin.model_result)
}

spec fn lifecycle_event_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1,
    event: LifecyclePairedEventV1) -> bool
{
    match event {
        LifecyclePairedEventV1::Mutation { actual, model } => {
            &&& lifecycle_step_inputs_v1(actual, model)
            &&& lifecycle_actual_step_relation_v1(before, after, actual)
            &&& logical::lifecycle_step_relation_v1(model_before, model_after, model)
        },
        LifecyclePairedEventV1::Observe { query, actual_answer, model_answer } => {
            &&& after == before && model_after == model_before
            &&& actual_answer == lifecycle_query_actual_v1(before, query)
            &&& model_answer == lifecycle_query_model_v1(model_before, query)
        },
    }
}

spec fn lifecycle_event_answers_v1(event: LifecyclePairedEventV1) -> bool {
    match event {
        LifecyclePairedEventV1::Mutation { actual, model } => lifecycle_step_answers_v1(actual, model),
        LifecyclePairedEventV1::Observe { actual_answer, model_answer, .. } => actual_answer == model_answer,
    }
}

spec fn lifecycle_event_history_v1(history: logical::LifecycleHistoryV1, event: LifecyclePairedEventV1)
    -> logical::LifecycleHistoryV1
{
    match event {
        LifecyclePairedEventV1::Mutation { model, .. } => logical::lifecycle_next_history_v1(history, model),
        LifecyclePairedEventV1::Observe { .. } => history,
    }
}

spec fn lifecycle_paired_history_v1(events: Seq<LifecyclePairedEventV1>, end: nat) -> logical::LifecycleHistoryV1
    recommends end <= events.len(),
    decreases end,
{
    if end == 0 { logical::lifecycle_empty_history_v1() }
    else { lifecycle_event_history_v1(lifecycle_paired_history_v1(events, (end - 1) as nat), events[end - 1]) }
}

spec fn lifecycle_paired_trace_v1(origin: LifecycleOriginV1, actual_states: Seq<ContextProducerReadJournalV1>,
    model_states: Seq<logical::ProducerReadContentsV1>, events: Seq<LifecyclePairedEventV1>) -> bool
{
    &&& lifecycle_origin_relation_v1(origin)
    // Trace admission excludes continuation after a constructor error; this is not a scheduler proof.
    &&& match origin.actual_result {
        Ok(owner) => actual_states.len() == events.len() + 1 && actual_states[0] == owner,
        Err(_) => actual_states.len() == 0 && events.len() == 0,
    }
    &&& match origin.model_result {
        Ok(owner) => model_states.len() == events.len() + 1 && model_states[0] == owner,
        Err(_) => model_states.len() == 0 && events.len() == 0,
    }
    &&& forall|i: int| 0 <= i < events.len() ==> lifecycle_event_relation_v1(
        actual_states[i], actual_states[i + 1], model_states[i], model_states[i + 1], #[trigger] events[i])
}

proof fn lifecycle_origin_correspondence_v1(origin: LifecycleOriginV1)
    requires lifecycle_origin_relation_v1(origin),
    ensures constructor_observations_match_v1(origin.actual_after, origin.model_after),
        constructor_producer_results_match_v1(origin.actual_result, origin.model_result),
{
    constructor_producer_paired_transition_v1(origin.actual_before, origin.actual_after,
        origin.model_before, origin.model_after, origin.context, origin.allocations, origin.writers,
        origin.reads, origin.actual_result, origin.model_result);
}

#[verifier::spinoff_prover]
proof fn lifecycle_event_preserves_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1,
    event: LifecyclePairedEventV1, storage: logical::StorageCapacitiesV1, history: logical::LifecycleHistoryV1)
    requires producer_represents(before, model_before), logical::lifecycle_invariant_v1(model_before, storage, history),
        lifecycle_event_relation_v1(before, after, model_before, model_after, event),
    ensures producer_represents(after, model_after),
        logical::lifecycle_invariant_v1(model_after, storage, lifecycle_event_history_v1(history, event)),
        logical::lifecycle_shape_v1(model_after) == logical::lifecycle_shape_v1(model_before),
        lifecycle_event_answers_v1(event),
{
    match event {
        LifecyclePairedEventV1::Mutation { actual, model } => {
            lifecycle_mutation_correspondence_v1(before, after, model_before, model_after, actual, model, storage, history.writers);
            logical::lifecycle_step_preserves_v1(model_before, model_after, model, storage, history);
            logical::lifecycle_step_shape_v1(model_before, model_after, model);
        },
        LifecyclePairedEventV1::Observe { query, .. } => {
            lifecycle_query_correspondence_v1(before, model_before, query);
        },
    }
}

#[verifier::spinoff_prover]
proof fn lifecycle_paired_prefix_v1(origin: LifecycleOriginV1, actual_states: Seq<ContextProducerReadJournalV1>,
    model_states: Seq<logical::ProducerReadContentsV1>, events: Seq<LifecyclePairedEventV1>,
    storage: logical::StorageCapacitiesV1, end: nat)
    requires lifecycle_paired_trace_v1(origin, actual_states, model_states, events),
        logical::storage_admission_v1(origin.allocations, origin.writers, storage), end < actual_states.len(),
    ensures producer_represents(actual_states[end as int], model_states[end as int]),
        logical::lifecycle_invariant_v1(model_states[end as int], storage, lifecycle_paired_history_v1(events, end)),
        logical::lifecycle_shape_v1(model_states[end as int]) ==
            (origin.context, origin.allocations, origin.writers, origin.reads as nat, origin.reads as nat),
        forall|i: int| 0 <= i < end ==> lifecycle_event_answers_v1(#[trigger] events[i]),
    decreases end,
{
    if end == 0 {
        lifecycle_origin_correspondence_v1(origin);
        assert(origin.actual_result.is_ok());
        assert(origin.model_result.is_ok());
        logical::lifecycle_constructor_preserves_v1(origin.model_before, origin.model_after, origin.context,
            origin.allocations, origin.writers, origin.reads, model_states[0], storage);
    } else {
        lifecycle_paired_prefix_v1(origin, actual_states, model_states, events, storage, (end - 1) as nat);
        lifecycle_event_preserves_v1(actual_states[end - 1], actual_states[end as int],
            model_states[end - 1], model_states[end as int], events[end - 1], storage,
            lifecycle_paired_history_v1(events, (end - 1) as nat));
        assert forall|i: int| 0 <= i < end implies lifecycle_event_answers_v1(#[trigger] events[i]) by {
            if i != end - 1 { assert(i < end - 1); }
        }
    }
}

// No successful admission is required: rejected operations and constructor failures are included.
#[verifier::spinoff_prover]
proof fn lifecycle_constructor_origin_trace_v1(origin: LifecycleOriginV1, actual_states: Seq<ContextProducerReadJournalV1>,
    model_states: Seq<logical::ProducerReadContentsV1>, events: Seq<LifecyclePairedEventV1>, storage: logical::StorageCapacitiesV1)
    requires lifecycle_paired_trace_v1(origin, actual_states, model_states, events),
        origin.actual_result.is_ok() ==> logical::storage_admission_v1(origin.allocations, origin.writers, storage),
    ensures constructor_observations_match_v1(origin.actual_after, origin.model_after),
        constructor_producer_results_match_v1(origin.actual_result, origin.model_result),
        actual_states.len() == model_states.len(),
        origin.actual_result.is_err() ==> actual_states.len() == 0 && model_states.len() == 0 && events.len() == 0,
        forall|i: int| 0 <= i < actual_states.len() ==> (
            producer_represents(#[trigger] actual_states[i], model_states[i])
            && logical::lifecycle_invariant_v1(model_states[i], storage, lifecycle_paired_history_v1(events, i as nat))
            && logical::lifecycle_shape_v1(model_states[i]) ==
                (origin.context, origin.allocations, origin.writers, origin.reads as nat, origin.reads as nat)),
        forall|i: int| 0 <= i < events.len() ==> lifecycle_event_answers_v1(#[trigger] events[i]),
{
    lifecycle_origin_correspondence_v1(origin);
    assert forall|i: int| 0 <= i < actual_states.len() implies (
        producer_represents(#[trigger] actual_states[i], model_states[i])
        && logical::lifecycle_invariant_v1(model_states[i], storage, lifecycle_paired_history_v1(events, i as nat))
        && logical::lifecycle_shape_v1(model_states[i]) ==
            (origin.context, origin.allocations, origin.writers, origin.reads as nat, origin.reads as nat)) by {
        lifecycle_paired_prefix_v1(origin, actual_states, model_states, events, storage, i as nat);
    }
    if origin.actual_result.is_ok() {
        lifecycle_paired_prefix_v1(origin, actual_states, model_states, events, storage, events.len());
    }
}

proof fn lifecycle_paired_history_nonreissue_v1(origin: LifecycleOriginV1, actual_states: Seq<ContextProducerReadJournalV1>,
    model_states: Seq<logical::ProducerReadContentsV1>, events: Seq<LifecyclePairedEventV1>,
    storage: logical::StorageCapacitiesV1, end: nat, first: int, last: int)
    requires lifecycle_paired_trace_v1(origin, actual_states, model_states, events),
        logical::storage_admission_v1(origin.allocations, origin.writers, storage), end < actual_states.len(), 0 <= first < last,
    ensures last < lifecycle_paired_history_v1(events, end).stable.len() ==>
            lifecycle_paired_history_v1(events, end).stable[first].incarnation
                < lifecycle_paired_history_v1(events, end).stable[last].incarnation,
        last < lifecycle_paired_history_v1(events, end).producer.len() ==>
            lifecycle_paired_history_v1(events, end).producer[first].incarnation
                < lifecycle_paired_history_v1(events, end).producer[last].incarnation,
{
    lifecycle_paired_prefix_v1(origin, actual_states, model_states, events, storage, end);
}

}

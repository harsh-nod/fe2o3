// Machine-sized inputs and constructor observation headroom are external input conditions.
verus! {

spec fn lifecycle_mutation_input_shape_v1(step: LifecycleActualStepV1) -> bool {
    match step {
        LifecycleActualStepV1::EnrollBatch { entries, original, .. } =>
            entries.len() <= usize::MAX && original.len() <= usize::MAX,
        LifecycleActualStepV1::Retire { roster, .. } => roster.len() <= usize::MAX,
        LifecycleActualStepV1::Begin { roster, .. } => roster.len() <= usize::MAX,
        LifecycleActualStepV1::AcquireStable { requests, original, .. } =>
            requests.len() <= usize::MAX && requests.len() <= u64::MAX && original.len() <= usize::MAX,
        LifecycleActualStepV1::ReleaseStable { references, .. } => references.len() <= usize::MAX,
        LifecycleActualStepV1::AcquireProducer { requests, original, .. } =>
            requests.len() <= usize::MAX && requests.len() <= u64::MAX && original.len() <= usize::MAX,
        LifecycleActualStepV1::AcquireMixed { stable, pending, stable_original, producer_original, .. } =>
            stable.len() <= usize::MAX && stable.len() <= u64::MAX && pending.len() <= usize::MAX && pending.len() <= u64::MAX
                && stable_original.len() <= usize::MAX && producer_original.len() <= usize::MAX,
        LifecycleActualStepV1::ReleaseProducer { references, .. } => references.len() <= usize::MAX,
        LifecycleActualStepV1::Dispose { roster, .. } => roster.len() <= usize::MAX,
        _ => true,
    }
}

spec fn lifecycle_mutation_domain_v1(owner: ContextProducerReadJournalV1, step: LifecycleActualStepV1) -> bool {
    &&& lifecycle_mutation_input_shape_v1(step)
    &&& match step {
        LifecycleActualStepV1::Retire { roster, .. } => retirement_producer_safe_v1(owner, roster, 0),
        LifecycleActualStepV1::Begin { .. } => producer_guard_storage_v1(owner),
        LifecycleActualStepV1::AcquireStable { consumer, requests, original, .. } =>
            producer_stable_acquire_domain_v1(owner, consumer, requests.len() as usize, original),
        LifecycleActualStepV1::ReleaseStable { .. } => stable_guard_storage_v1(owner.stable),
        LifecycleActualStepV1::AcquireProducer { consumer, requests, original, .. } =>
            producer_acquire_domain_v1(owner, consumer, requests.len() as usize, original),
        LifecycleActualStepV1::AcquireMixed { .. } => mixed_acquire_storage_v1(owner),
        LifecycleActualStepV1::ReleaseProducer { consumer, references, evidence, capacity, .. } =>
            producer_release_domain_v1(owner, consumer, evidence, references.len() as usize, capacity),
        LifecycleActualStepV1::Dispose { roster, .. } => disposal_producer_safe_v1(owner, roster, 0),
        _ => true,
    }
}

proof fn lifecycle_mixed_domain_projection_v1(owner: ContextProducerReadJournalV1, step: LifecycleActualStepV1)
    requires lifecycle_mutation_domain_v1(owner, step), matches!(step, LifecycleActualStepV1::AcquireMixed { .. }),
    ensures mixed_acquire_storage_v1(owner),
{
}

spec fn lifecycle_query_input_shape_v1(query: LifecycleQueryV1) -> bool {
    match query {
        LifecycleQueryV1::StableCapacity(count) | LifecycleQueryV1::CombinedStableCapacity(count)
        | LifecycleQueryV1::ProducerCapacity(count) => count <= u64::MAX,
        LifecycleQueryV1::ValidateRetirement { roster, .. } => roster.len() <= usize::MAX,
        LifecycleQueryV1::ValidateDisposal { roster, .. } => roster.len() <= usize::MAX,
        _ => true,
    }
}

spec fn lifecycle_query_domain_v1(owner: ContextProducerReadJournalV1, query: LifecycleQueryV1) -> bool {
    &&& lifecycle_query_input_shape_v1(query)
    &&& match query {
        LifecycleQueryV1::Getter(_) => producer_budget_domain_v1(owner) && producer_total_retained_v1(owner) <= usize::MAX,
        LifecycleQueryV1::ReaderCount { producer, allocation } => if producer {
            producer_count_safe_v1(owner, allocation) } else { stable_count_safe_v1(owner.stable, allocation) },
        LifecycleQueryV1::CombinedStableCapacity(_) | LifecycleQueryV1::ProducerCapacity(_) => producer_budget_domain_v1(owner),
        LifecycleQueryV1::ValidateRetirement { receiver, roster, .. } => match receiver {
            LifecycleReceiverV1::Journal => true,
            LifecycleReceiverV1::Stable => retirement_stable_safe_v1(owner.stable, roster, 0),
            LifecycleReceiverV1::Producer => retirement_producer_safe_v1(owner, roster, 0),
        },
        LifecycleQueryV1::ValidateDisposal { receiver, roster, .. } => match receiver {
            LifecycleReceiverV1::Journal => true,
            LifecycleReceiverV1::Stable => disposal_stable_safe_v1(owner.stable, roster, 0),
            LifecycleReceiverV1::Producer => disposal_producer_safe_v1(owner, roster, 0),
        },
        _ => true,
    }
}

proof fn lifecycle_represented_storage_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1)
    requires producer_represents(actual, model), logical::producer_invariant_v1(model),
    ensures producer_guard_storage_v1(actual), producer_budget_domain_v1(actual),
        0 <= producer_total_retained_v1(actual) <= usize::MAX,
{
    producer_guard_storage_from_model(actual, model);
    logical::producer_capacity_arithmetic_v1(model);
}

proof fn lifecycle_stable_retirement_domain_v1(owner: ContextReadLeasedJournalV1,
    roster: Seq<AllocationReferenceV1>, index: nat)
    requires stable_guard_storage_v1(owner),
    ensures retirement_stable_safe_v1(owner, roster, index),
    decreases roster.len() - index,
{
    if index < roster.len() {
        if allocation_lookup_decision_v1(owner.journal, roster[index as int]).is_ok() {
            assert(roster[index as int].slot < owner.journal.allocations@.len());
        }
        lifecycle_stable_retirement_domain_v1(owner, roster, index + 1);
    }
}

proof fn lifecycle_stable_disposal_domain_v1(owner: ContextReadLeasedJournalV1,
    roster: Seq<AllocationWriteV1>, index: nat)
    requires stable_guard_storage_v1(owner),
    ensures disposal_stable_safe_v1(owner, roster, index),
    decreases roster.len() - index,
{
    if index < roster.len() {
        if allocation_lookup_decision_v1(owner.journal, roster[index as int].allocation).is_ok() {
            assert(roster[index as int].allocation.slot < owner.journal.allocations@.len());
        }
        lifecycle_stable_disposal_domain_v1(owner, roster, index + 1);
    }
}

#[verifier::spinoff_prover]
proof fn lifecycle_mutation_domain_from_reached_v1(actual: ContextProducerReadJournalV1,
    model: logical::ProducerReadContentsV1, step: LifecycleActualStepV1)
    requires producer_represents(actual, model), logical::producer_invariant_v1(model), lifecycle_mutation_input_shape_v1(step),
    ensures lifecycle_mutation_domain_v1(actual, step),
{
    lifecycle_represented_storage_v1(actual, model);
    match step {
        LifecycleActualStepV1::Retire { roster, .. } => owner_retirement_invariant_domain_v1(actual, model, roster),
        LifecycleActualStepV1::Dispose { roster, .. } => owner_disposal_invariant_domain_v1(actual, model, roster),
        LifecycleActualStepV1::AcquireMixed { .. } => mixed_acquire_storage_from_model_v1(actual, model),
        _ => {},
    }
}

#[verifier::spinoff_prover]
proof fn lifecycle_query_domain_from_reached_v1(actual: ContextProducerReadJournalV1,
    model: logical::ProducerReadContentsV1, query: LifecycleQueryV1)
    requires producer_represents(actual, model), logical::producer_invariant_v1(model), lifecycle_query_input_shape_v1(query),
    ensures lifecycle_query_domain_v1(actual, query),
{
    lifecycle_represented_storage_v1(actual, model);
    match query {
        LifecycleQueryV1::ReaderCount { allocation, .. } => {
            if allocation_lookup_decision_v1(actual.stable.journal, allocation).is_ok() {
                assert(allocation.slot < actual.stable.journal.allocations@.len());
            }
        },
        LifecycleQueryV1::ValidateRetirement { roster, .. } => {
            owner_retirement_invariant_domain_v1(actual, model, roster);
            lifecycle_stable_retirement_domain_v1(actual.stable, roster, 0);
        },
        LifecycleQueryV1::ValidateDisposal { roster, .. } => {
            owner_disposal_invariant_domain_v1(actual, model, roster);
            lifecycle_stable_disposal_domain_v1(actual.stable, roster, 0);
        },
        _ => {},
    }
}

spec fn lifecycle_origin_input_shape_v1(origin: LifecycleOriginV1) -> bool {
    origin.actual_before.attempts@.len() + 13 <= usize::MAX
}

spec fn lifecycle_event_input_shape_v1(event: LifecyclePairedEventV1) -> bool {
    match event {
        LifecyclePairedEventV1::Mutation { actual, .. } => lifecycle_mutation_input_shape_v1(actual),
        LifecyclePairedEventV1::Observe { query, .. } => lifecycle_query_input_shape_v1(query),
    }
}

spec fn lifecycle_event_domain_v1(owner: ContextProducerReadJournalV1, event: LifecyclePairedEventV1) -> bool {
    match event {
        LifecyclePairedEventV1::Mutation { actual, .. } => lifecycle_mutation_domain_v1(owner, actual),
        LifecyclePairedEventV1::Observe { query, .. } => lifecycle_query_domain_v1(owner, query),
    }
}

// This is a contract-composition corollary, not an executable trace interpreter.
#[verifier::spinoff_prover]
proof fn lifecycle_reached_operation_domains_v1(origin: LifecycleOriginV1, actual_states: Seq<ContextProducerReadJournalV1>,
    model_states: Seq<logical::ProducerReadContentsV1>, events: Seq<LifecyclePairedEventV1>, storage: logical::StorageCapacitiesV1)
    requires lifecycle_paired_trace_v1(origin, actual_states, model_states, events),
        origin.actual_result.is_ok() ==> logical::storage_admission_v1(origin.allocations, origin.writers, storage),
        lifecycle_origin_input_shape_v1(origin),
        forall|i: int| 0 <= i < events.len() ==> lifecycle_event_input_shape_v1(#[trigger] events[i]),
    ensures constructor_observations_match_v1(origin.actual_before, origin.model_before),
        origin.actual_before.attempts@.len() + 13 <= usize::MAX,
        origin.model_before.attempts@.len() + 13 <= usize::MAX,
        forall|i: int| 0 <= i < events.len() ==> lifecycle_event_domain_v1(actual_states[i], #[trigger] events[i]),
{
    lifecycle_constructor_origin_trace_v1(origin, actual_states, model_states, events, storage);
    assert forall|i: int| 0 <= i < events.len() implies lifecycle_event_domain_v1(actual_states[i], #[trigger] events[i]) by {
        assert(origin.actual_result.is_ok());
        lifecycle_paired_prefix_v1(origin, actual_states, model_states, events, storage, i as nat);
        match events[i] {
            LifecyclePairedEventV1::Mutation { actual, .. } => {
                lifecycle_mutation_domain_from_reached_v1(actual_states[i], model_states[i], actual);
            },
            LifecyclePairedEventV1::Observe { query, .. } => {
                lifecycle_query_domain_from_reached_v1(actual_states[i], model_states[i], query);
            },
        }
    }
}

}

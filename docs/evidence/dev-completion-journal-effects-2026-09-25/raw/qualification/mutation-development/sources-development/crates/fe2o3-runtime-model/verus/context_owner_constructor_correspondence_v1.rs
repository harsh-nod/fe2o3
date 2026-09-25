verus! {

spec fn constructor_error_embed_v1(error: logical::ConstructorErrorV1) -> ContextVersionJournalErrorV1 {
    match error {
        logical::ConstructorErrorV1::InvalidContextGeneration => ContextVersionJournalErrorV1::InvalidContextGeneration,
        logical::ConstructorErrorV1::InvalidCapacity => ContextVersionJournalErrorV1::InvalidCapacity,
        logical::ConstructorErrorV1::StorageAllocationFailed => ContextVersionJournalErrorV1::StorageAllocationFailed,
    }
}

spec fn constructor_observations_match_v1(actual: ConstructorObservationsV1, model: logical::ConstructorObservationsV1) -> bool {
    actual.outcomes@ == model.outcomes@ && actual.attempts@ == model.attempts@
}

proof fn constructor_trace_success_at_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    requests: Seq<(u8, usize)>, success: bool, index: int)
    requires constructor_trace_v1(before, after, requests, success),
        0 <= index < after.attempts@.len() - before.attempts@.len(),
        index + 1 < after.attempts@.len() - before.attempts@.len() || success,
    ensures constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + index) as nat),
{}

proof fn constructor_model_trace_success_at_v1(before: logical::ConstructorObservationsV1, after: logical::ConstructorObservationsV1,
    requests: Seq<(u8, usize)>, success: bool, index: int)
    requires logical::constructor_trace_v1(before, after, requests, success),
        0 <= index < after.attempts@.len() - before.attempts@.len(),
        index + 1 < after.attempts@.len() - before.attempts@.len() || success,
    ensures logical::constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + index) as nat),
{}

// A failed prefix cannot also be a successful proper prefix of a longer trace.
proof fn constructor_trace_paired_unique_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    model_before: logical::ConstructorObservationsV1, model_after: logical::ConstructorObservationsV1,
    requests: Seq<(u8, usize)>, success: bool, model_success: bool)
    requires constructor_observations_match_v1(before, model_before),
        constructor_trace_v1(before, after, requests, success),
        logical::constructor_trace_v1(model_before, model_after, requests, model_success),
    ensures constructor_observations_match_v1(after, model_after), success == model_success,
{
    let count = (after.attempts@.len() - before.attempts@.len()) as nat;
    let model_count = (model_after.attempts@.len() - model_before.attempts@.len()) as nat;
    if count < model_count {
        assert(!success && count > 0);
        constructor_model_trace_success_at_v1(model_before, model_after, requests, model_success, count - 1);
        assert(logical::constructor_outcome_v1(model_before.outcomes@, (model_before.attempts@.len() + count - 1) as nat));
        assert(!constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + count - 1) as nat));
        assert(false);
    }
    if model_count < count {
        assert(!model_success && model_count > 0);
        constructor_trace_success_at_v1(before, after, requests, success, model_count - 1);
        assert(constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + model_count - 1) as nat));
        assert(!logical::constructor_outcome_v1(model_before.outcomes@, (model_before.attempts@.len() + model_count - 1) as nat));
        assert(false);
    }
    assert(count == model_count);
    if success && !model_success {
        assert(count > 0);
        constructor_trace_success_at_v1(before, after, requests, success, count - 1);
        assert(constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + count - 1) as nat));
        assert(false);
    }
    if !success && model_success {
        assert(count > 0);
        constructor_model_trace_success_at_v1(model_before, model_after, requests, model_success, count - 1);
        assert(logical::constructor_outcome_v1(model_before.outcomes@, (model_before.attempts@.len() + count - 1) as nat));
        assert(false);
    }
}

proof fn constructor_requests_correspondence_v1(allocations: usize, writers: usize, reads: usize)
    ensures constructor_requests_v1(allocations, writers, reads) == logical::constructor_requests_v1(allocations, writers, reads),
{
    assert(constructor_requests_v1(allocations, writers, reads) =~= logical::constructor_requests_v1(allocations, writers, reads));
}

proof fn constructor_journal_initialized_correspondence_v1(actual: ContextVersionJournalV1, model: logical::JournalContentsV1,
    context: u64, allocations: usize, writers: usize)
    requires constructor_journal_initialized_v1(actual, context, allocations, writers),
        logical::constructor_contents_initialized_v1(model, context, allocations, writers),
    ensures represents(actual, model),
{
    assert(journal_view(actual).writers =~= model.writers@);
    assert(journal_view(actual).allocations =~= model.allocations@);
    assert(journal_view(actual).members =~= model.members@);
    assert(journal_view(actual).scratch =~= model.scratch@);
}

proof fn constructor_stable_initialized_correspondence_v1(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    context: u64, allocations: usize, writers: usize, reads: usize)
    requires constructor_stable_initialized_v1(actual, context, allocations, writers, reads),
        logical::reader_constructor_relation_v1(context, allocations, writers, reads, Ok(model)),
    ensures stable_represents(actual, model),
{
    constructor_journal_initialized_correspondence_v1(actual.journal, model.journal, context, allocations, writers);
    assert(actual.leases@.map(|_i, entry| read_lease_slot_view(entry)) =~= model.leases@);
}

proof fn constructor_producer_initialized_correspondence_v1(actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1,
    context: u64, allocations: usize, writers: usize, reads: usize)
    requires constructor_producer_initialized_v1(actual, context, allocations, writers, reads),
        logical::producer_constructor_relation_v1(context, allocations, writers, reads, Ok(model)),
    ensures producer_represents(actual, model),
{
    constructor_stable_initialized_correspondence_v1(actual.stable, model.stable, context, allocations, writers, reads);
    assert(actual.reservations@.map(|_i, entry| producer_reservation_slot_view(entry)) =~= model.reservations@);
}

spec fn constructor_journal_results_match_v1(actual: Result<ContextVersionJournalV1, ContextVersionJournalErrorV1>,
    model: Result<logical::JournalContentsV1, logical::ConstructorErrorV1>) -> bool {
    match (actual, model) {
        (Ok(value), Ok(contents)) => represents(value, contents),
        (Err(error), Err(model_error)) => error == constructor_error_embed_v1(model_error),
        _ => false,
    }
}

spec fn constructor_stable_results_match_v1(actual: Result<ContextReadLeasedJournalV1, ContextVersionJournalErrorV1>,
    model: Result<logical::ReadContentsV1, logical::ConstructorErrorV1>) -> bool {
    match (actual, model) {
        (Ok(value), Ok(contents)) => stable_represents(value, contents),
        (Err(error), Err(model_error)) => error == constructor_error_embed_v1(model_error),
        _ => false,
    }
}

spec fn constructor_producer_results_match_v1(actual: Result<ContextProducerReadJournalV1, ContextVersionJournalErrorV1>,
    model: Result<logical::ProducerReadContentsV1, logical::ConstructorErrorV1>) -> bool {
    match (actual, model) {
        (Ok(value), Ok(contents)) => producer_represents(value, contents),
        (Err(error), Err(model_error)) => error == constructor_error_embed_v1(model_error),
        _ => false,
    }
}

proof fn constructor_journal_paired_transition_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    model_before: logical::ConstructorObservationsV1, model_after: logical::ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize,
    result: Result<ContextVersionJournalV1, ContextVersionJournalErrorV1>,
    model_result: Result<logical::JournalContentsV1, logical::ConstructorErrorV1>)
    requires constructor_observations_match_v1(before, model_before),
        constructor_journal_relation_v1(before, after, context, allocations, writers, result),
        logical::constructor_journal_relation_v1(model_before, model_after, context, allocations, writers, model_result),
    ensures constructor_observations_match_v1(after, model_after), constructor_journal_results_match_v1(result, model_result),
{
    if constructor_journal_admission_v1(context, allocations, writers).is_none() {
        constructor_requests_correspondence_v1(allocations, writers, 0);
        constructor_trace_paired_unique_v1(before, after, model_before, model_after,
            constructor_requests_v1(allocations, writers, 0).take(7), result.is_ok(), model_result.is_ok());
        if let (Ok(value), Ok(contents)) = (result, model_result) {
            constructor_journal_initialized_correspondence_v1(value, contents, context, allocations, writers);
        }
    }
}

proof fn constructor_stable_paired_transition_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    model_before: logical::ConstructorObservationsV1, model_after: logical::ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize,
    result: Result<ContextReadLeasedJournalV1, ContextVersionJournalErrorV1>,
    model_result: Result<logical::ReadContentsV1, logical::ConstructorErrorV1>)
    requires constructor_observations_match_v1(before, model_before),
        constructor_stable_relation_v1(before, after, context, allocations, writers, reads, result),
        logical::constructor_stable_relation_v1(model_before, model_after, context, allocations, writers, reads, model_result),
    ensures constructor_observations_match_v1(after, model_after), constructor_stable_results_match_v1(result, model_result),
{
    if constructor_owner_admission_v1(context, allocations, writers, reads).is_none() {
        constructor_requests_correspondence_v1(allocations, writers, reads);
        constructor_trace_paired_unique_v1(before, after, model_before, model_after,
            constructor_requests_v1(allocations, writers, reads).take(10), result.is_ok(), model_result.is_ok());
        if let (Ok(value), Ok(contents)) = (result, model_result) {
            constructor_stable_initialized_correspondence_v1(value, contents, context, allocations, writers, reads);
        }
    }
}

proof fn constructor_producer_paired_transition_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    model_before: logical::ConstructorObservationsV1, model_after: logical::ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize,
    result: Result<ContextProducerReadJournalV1, ContextVersionJournalErrorV1>,
    model_result: Result<logical::ProducerReadContentsV1, logical::ConstructorErrorV1>)
    requires constructor_observations_match_v1(before, model_before),
        constructor_producer_relation_v1(before, after, context, allocations, writers, reads, result),
        logical::constructor_producer_relation_v1(model_before, model_after, context, allocations, writers, reads, model_result),
    ensures constructor_observations_match_v1(after, model_after), constructor_producer_results_match_v1(result, model_result),
{
    if constructor_owner_admission_v1(context, allocations, writers, reads).is_none() {
        constructor_requests_correspondence_v1(allocations, writers, reads);
        constructor_trace_paired_unique_v1(before, after, model_before, model_after,
            constructor_requests_v1(allocations, writers, reads), result.is_ok(), model_result.is_ok());
        if let (Ok(value), Ok(contents)) = (result, model_result) {
            constructor_producer_initialized_correspondence_v1(value, contents, context, allocations, writers, reads);
        }
    }
}

fn constructor_journal_paired_exec_v1(actual: &mut ConstructorObservationsV1, model: &mut logical::ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize)
    -> (results: (Result<ContextVersionJournalV1, ContextVersionJournalErrorV1>, Result<logical::JournalContentsV1, logical::ConstructorErrorV1>))
    requires constructor_observations_match_v1(*old(actual), *old(model)), old(actual).attempts@.len() + 7 <= usize::MAX,
    ensures constructor_observations_match_v1(*final(actual), *final(model)), constructor_journal_results_match_v1(results.0, results.1),
        constructor_journal_relation_v1(*old(actual), *final(actual), context, allocations, writers, results.0),
        logical::constructor_journal_relation_v1(*old(model), *final(model), context, allocations, writers, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = ContextVersionJournalV1::new_observed_v1(context, allocations, writers, actual);
    let model_result = logical::constructor_journal_model_exec_v1(model, context, allocations, writers);
    proof { constructor_journal_paired_transition_v1(before, *actual, model_before, *model, context, allocations, writers, result, model_result); }
    (result, model_result)
}

fn constructor_stable_paired_exec_v1(actual: &mut ConstructorObservationsV1, model: &mut logical::ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize)
    -> (results: (Result<ContextReadLeasedJournalV1, ContextVersionJournalErrorV1>, Result<logical::ReadContentsV1, logical::ConstructorErrorV1>))
    requires constructor_observations_match_v1(*old(actual), *old(model)), old(actual).attempts@.len() + 10 <= usize::MAX,
    ensures constructor_observations_match_v1(*final(actual), *final(model)), constructor_stable_results_match_v1(results.0, results.1),
        constructor_stable_relation_v1(*old(actual), *final(actual), context, allocations, writers, reads, results.0),
        logical::constructor_stable_relation_v1(*old(model), *final(model), context, allocations, writers, reads, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = ContextReadLeasedJournalV1::new_observed_v1(context, allocations, writers, reads, actual);
    let model_result = logical::constructor_stable_model_exec_v1(model, context, allocations, writers, reads);
    proof { constructor_stable_paired_transition_v1(before, *actual, model_before, *model, context, allocations, writers, reads, result, model_result); }
    (result, model_result)
}

fn constructor_producer_paired_exec_v1(actual: &mut ConstructorObservationsV1, model: &mut logical::ConstructorObservationsV1,
    context: u64, allocations: usize, writers: usize, reads: usize, Ghost(storage): Ghost<logical::StorageCapacitiesV1>)
    -> (results: (Result<ContextProducerReadJournalV1, ContextVersionJournalErrorV1>, Result<logical::ProducerReadContentsV1, logical::ConstructorErrorV1>))
    requires constructor_observations_match_v1(*old(actual), *old(model)), old(actual).attempts@.len() + 13 <= usize::MAX,
    ensures constructor_observations_match_v1(*final(actual), *final(model)), constructor_producer_results_match_v1(results.0, results.1),
        constructor_producer_relation_v1(*old(actual), *final(actual), context, allocations, writers, reads, results.0),
        logical::constructor_producer_relation_v1(*old(model), *final(model), context, allocations, writers, reads, results.1),
        match results.1 {
            Ok(contents) => logical::producer_invariant_v1(contents)
                && (logical::storage_admission_v1(allocations, writers, storage)
                    ==> logical::issued_producer_v1(contents, storage, Seq::empty())),
            Err(_) => true,
        },
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = ContextProducerReadJournalV1::new_observed_v1(context, allocations, writers, reads, actual);
    let model_result = logical::constructor_producer_model_exec_v1(model, context, allocations, writers, reads);
    let ghost constructed = model_result;
    proof {
        constructor_producer_paired_transition_v1(before, *actual, model_before, *model, context, allocations, writers, reads, result, model_result);
        if let Ok(contents) = constructed {
            logical::constructor_producer_issued_v1(model_before, *model, context, allocations, writers, reads, contents, storage);
        }
    }
    (result, model_result)
}

}

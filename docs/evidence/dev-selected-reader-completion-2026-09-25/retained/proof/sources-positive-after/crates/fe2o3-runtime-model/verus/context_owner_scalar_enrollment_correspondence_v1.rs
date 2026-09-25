verus! {

spec fn scalar_enrollment_result_from_v1(result: Result<logical::AllocationReferenceV1, logical::EnrollmentErrorV1>)
    -> Result<AllocationReferenceV1, ReadErrorV1>
{
    match result { Ok(reference) => Ok(allocation_reference_from(reference)), Err(error) => Err(enrollment_error_embed(error)) }
}

proof fn scalar_enrollment_decision_correspondence_v1(before: JournalContentsV1, model: logical::JournalContentsV1,
    entry: EnrollmentV1)
    requires represents(before, model),
    ensures scalar_enrollment_decision_v1(before, entry.key, entry.device, entry.byte_extent)
        == scalar_enrollment_result_from_v1(logical::scalar_enrollment_decision_v1(model, enrollment_view(entry))),
{
    let key = entry.key;
    assert forall|i: int| 0 <= i < before.allocations@.len() implies
        ((#[trigger] before.allocations@[i]).is_some() && before.allocations@[i].unwrap().key == key)
        == (model.allocations@[i].is_some() && model.allocations@[i].unwrap().key == allocation_key_view(key)) by {
        if before.allocations@[i].is_some() {
            enrollment_order_correspondence(before.allocations@[i].unwrap().key, key);
        }
    }
    allocation_key_round_trip(key, allocation_key_view(key));
}

proof fn scalar_enrollment_paired_transition_v1(before: JournalContentsV1, after: JournalContentsV1,
    model_before: logical::JournalContentsV1, model_after: logical::JournalContentsV1, entry: EnrollmentV1,
    result: Result<AllocationReferenceV1, ReadErrorV1>,
    model_result: Result<logical::AllocationReferenceV1, logical::EnrollmentErrorV1>)
    requires represents(before, model_before),
        scalar_enrollment_relation_v1(before, after, entry.key, entry.device, entry.byte_extent, result),
        logical::scalar_enrollment_relation_v1(model_before, model_after, enrollment_view(entry), model_result),
    ensures result == scalar_enrollment_result_from_v1(model_result), represents(after, model_after),
{
    scalar_enrollment_decision_correspondence_v1(before, model_before, entry);
    enrollment_entry_correspondence(before.context_generation, entry);
    if let Ok(reference) = result {
        allocation_reference_round_trip(reference, allocation_reference_view(reference));
        assert(journal_view(after).allocations =~= model_after.allocations@);
    }
}

// Raw correspondence has no custody or successful-admission premise.
fn owner_scalar_enrollment_paired_exec_v1(actual: &mut ContextProducerReadJournalV1,
    model: &mut logical::ProducerReadContentsV1, entry: EnrollmentV1, model_entry: logical::EnrollmentV1,
    Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (results: (Result<AllocationReferenceV1, ReadErrorV1>, Result<logical::AllocationReferenceV1, logical::EnrollmentErrorV1>))
    requires producer_represents(*old(actual), *old(model)), enrollment_view(entry) == model_entry,
    ensures results.0 == scalar_enrollment_result_from_v1(results.1), producer_represents(*final(actual), *final(model)),
        scalar_enrollment_relation_v1(old(actual).stable.journal, final(actual).stable.journal,
            entry.key, entry.device, entry.byte_extent, results.0),
        logical::scalar_enrollment_relation_v1(old(model).stable.journal, final(model).stable.journal, model_entry, results.1),
        owner_writer_producer_frame_v1(*old(actual), *final(actual)), owner_model_outer_frame_v1(*old(model), *final(model)),
        results.0.is_err() ==> *final(actual) == *old(actual), results.1.is_err() ==> *final(model) == *old(model),
        logical::producer_invariant_v1(*old(model)) ==> logical::producer_invariant_v1(*final(model)),
        logical::issued_producer_v1(*old(model), storage, history) ==> logical::issued_producer_v1(*final(model), storage, history),
        forall|request: logical::ProducerReadV1| logical::producer_status_v1(old(model).stable.journal, request).is_some()
            ==> #[trigger] logical::producer_status_v1(final(model).stable.journal, request)
                == logical::producer_status_v1(old(model).stable.journal, request),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = actual.enroll_allocation(entry.key, entry.device, entry.byte_extent);
    let model_result = logical::scalar_enrollment_exec_v1(&mut model.stable.journal, model_entry);
    proof {
        scalar_enrollment_paired_transition_v1(before.stable.journal, actual.stable.journal,
            model_before.stable.journal, model.stable.journal, entry, result, model_result);
        logical::scalar_enrollment_preserves_v1(model_before, *model, model_entry, model_result, storage, history);
    }
    (result, model_result)
}

}

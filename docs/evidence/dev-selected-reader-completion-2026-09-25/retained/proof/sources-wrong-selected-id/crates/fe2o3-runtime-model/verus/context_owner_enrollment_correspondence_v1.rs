verus! {

// Execute the public owner method and the historical journal independently.
fn producer_enrollment_historical_exec_v1(actual: &mut ContextProducerReadJournalV1,
    model: &mut logical::ProducerReadContentsV1, entries: &[EnrollmentV1], model_entries: &[logical::EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>], model_output: &mut [Option<logical::AllocationReferenceV1>])
    -> (results: (Result<(), EnrollmentErrorV1>, Result<(), logical::EnrollmentErrorV1>))
    requires producer_represents(*old(actual), *old(model)), entries_view(entries@) == model_entries@,
        references_view(old(output)@) == old(model_output)@,
    ensures results.0 == enrollment_result_from(results.1), producer_represents(*final(actual), *final(model)),
        references_view(final(output)@) == final(model_output)@,
        producer_enrollment_relation_v1(*old(actual), *final(actual), entries@, old(output)@, final(output)@, results.0),
        logical::enrollment_execution_relation_v1(old(model).stable.journal, final(model).stable.journal,
            model_entries@, old(model_output)@, final(model_output)@, results.1),
        logical::producer_storage_frame_v1(*old(model), *final(model)),
        final(model).reservations == old(model).reservations,
        final(model).free == old(model).free,
        final(model).counts == old(model).counts,
        final(model).next_incarnation == old(model).next_incarnation,
        final(model).stable.leases == old(model).stable.leases,
        final(model).stable.free_reads == old(model).stable.free_reads,
        final(model).stable.readers == old(model).stable.readers,
        final(model).stable.next_incarnation == old(model).stable.next_incarnation,
        results.0.is_err() || entries.len() == 0 ==> *final(actual) == *old(actual),
        results.1.is_err() ==> *final(model) == *old(model),
        logical::producer_invariant_v1(*old(model)) ==> logical::producer_invariant_v1(*final(model)),
        logical::producer_invariant_v1(*old(model)) ==> forall|request: logical::ProducerReadV1|
            logical::producer_status_v1(old(model).stable.journal, request).is_some()
            ==> #[trigger] logical::producer_status_v1(final(model).stable.journal, request)
                == logical::producer_status_v1(old(model).stable.journal, request),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost original = output@;
    let ghost model_original = model_output@;
    let result = actual.enroll_allocations(entries, output);
    let model_result = logical::enrollment_journal_exec_v1(&mut model.stable.journal, model_entries, model_output);
    proof {
        enrollment_paired_transition(before.stable.journal, actual.stable.journal,
            model_before.stable.journal, model.stable.journal, entries@, original,
            output@, model_output@, result, model_result);
        if logical::producer_invariant_v1(model_before) {
            logical::owner_enrollment_preserves_producer_v1(model_before, *model, model_entries@,
                model_original, model_output@, model_result);
        }
    }
    (result, model_result)
}

}

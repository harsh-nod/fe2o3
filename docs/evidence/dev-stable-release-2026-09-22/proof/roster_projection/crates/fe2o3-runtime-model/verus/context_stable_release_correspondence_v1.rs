verus! {

spec fn stable_references_view(references: Seq<ContextReadLeaseReferenceV1>) -> Seq<logical::ReadReferenceV1> {
    Seq::<logical::ReadReferenceV1>::empty()
}

spec fn stable_read_result_from(result: Result<logical::AllocationReadV1, logical::ReadErrorV1>)
    -> Result<ContextAllocationReadV1, ReadErrorV1> {
    match result { Err(error) => Err(read_error_embed(error)), Ok(request) => Ok(allocation_read_from(request)) }
}

spec fn stable_release_scan_view(state: StableReadReleaseScanV1) -> logical::ReadScanV1 {
    logical::ReadScanV1 { previous: state.previous, group: state.group }
}

proof fn stable_lease_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    reference: ContextReadLeaseReferenceV1)
    requires stable_represents(actual, model),
    ensures stable_lease_decision_v1(actual, reference)
        == stable_read_result_from(logical::lease_decision_v1(model, read_reference_view(reference))),
{
    if reference.slot < actual.leases@.len() {
        if let Some(entry) = actual.leases@[reference.slot as int] {
            stable_read_correspondence(actual.journal, model.journal, entry.request);
            allocation_read_round_trip(entry.request, allocation_read_view(entry.request));
        }
    }
}

proof fn stable_release_header_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    consumer: WriterKeyV1, evidence: WriterKeyV1, count: usize, observed_free_capacity: usize)
    requires stable_represents(actual, model),
    ensures stable_release_header_v1(actual, consumer, evidence, count, observed_free_capacity)
        == begin_result_from(logical::release_header_v1(model, writer_key_view(consumer), writer_key_view(evidence),
            count, observed_free_capacity)),
{}

fn stable_lookup_historical_exec_v1(actual: &ContextReadLeasedJournalV1, model: &logical::ReadContentsV1,
    reference: ContextReadLeaseReferenceV1, model_reference: logical::ReadReferenceV1)
    -> (results: (Result<ContextAllocationReadV1, ReadErrorV1>, Result<logical::AllocationReadV1, logical::ReadErrorV1>))
    requires stable_represents(*actual, *model), read_reference_view(reference) == model_reference,
    ensures results.0 == stable_read_result_from(results.1),
        results.0 == stable_lease_decision_v1(*actual, reference),
        results.1 == logical::lease_decision_v1(*model, model_reference),
{
    let result = actual.lookup_read(reference);
    let model_result = logical::lease_lookup_exec_v1(model, model_reference);
    proof { stable_lease_correspondence(*actual, *model, reference); }
    (result, model_result)
}

proof fn stable_release_item_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    consumer: WriterKeyV1, reference: ContextReadLeaseReferenceV1, state: StableReadReleaseScanV1)
    requires stable_represents(actual, model),
    ensures match logical::release_item_v1(model, writer_key_view(consumer), read_reference_view(reference), stable_release_scan_view(state)) {
        Err(error) => stable_release_item_v1(actual, consumer, reference, state) == Err(read_error_embed(error)),
        Ok(next) => stable_release_item_v1(actual, consumer, reference, state).is_ok()
            && stable_release_scan_view(stable_release_item_v1(actual, consumer, reference, state).unwrap()) == next,
    },
{
    stable_lease_correspondence(actual, model, reference);
}

proof fn stable_release_scan_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    consumer: WriterKeyV1, references: Seq<ContextReadLeaseReferenceV1>, index: nat, state: StableReadReleaseScanV1)
    requires stable_represents(actual, model),
    ensures stable_release_scan_v1(actual, consumer, references, index, state)
        == begin_result_from(logical::release_scan_v1(model, writer_key_view(consumer), stable_references_view(references),
            index, stable_release_scan_view(state))),
    decreases references.len() - index,
{
    if index < references.len() {
        stable_release_item_correspondence(actual, model, consumer, references[index as int], state);
        if let Ok(next) = stable_release_item_v1(actual, consumer, references[index as int], state) {
            stable_release_scan_correspondence(actual, model, consumer, references, index + 1, next);
        }
    }
}

proof fn stable_release_decision_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    consumer: WriterKeyV1, references: Seq<ContextReadLeaseReferenceV1>, evidence: WriterKeyV1, observed_free_capacity: usize)
    requires stable_represents(actual, model),
    ensures stable_release_decision_v1(actual, consumer, references, evidence, observed_free_capacity)
        == begin_result_from(logical::release_decision_v1(model, writer_key_view(consumer), stable_references_view(references),
            writer_key_view(evidence), observed_free_capacity)),
{
    stable_release_header_correspondence(actual, model, consumer, evidence, references.len() as usize, observed_free_capacity);
    stable_release_scan_correspondence(actual, model, consumer, references, 0, StableReadReleaseScanV1 { previous: None, group: 0 });
}

proof fn stable_released_requests_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    references: Seq<ContextReadLeaseReferenceV1>)
    requires stable_represents(actual, model), stable_release_commit_ready_v1(actual, references),
    ensures stable_requests_view(stable_released_requests_v1(actual, references))
        == logical::released_requests_v1(model, stable_references_view(references)),
{
    assert(stable_requests_view(stable_released_requests_v1(actual, references))
        =~= logical::released_requests_v1(model, stable_references_view(references)));
}

proof fn stable_released_leases_correspondence(actual: ContextReadLeasedJournalV1, model: logical::ReadContentsV1,
    references: Seq<ContextReadLeaseReferenceV1>, count: nat)
    requires stable_represents(actual, model), stable_release_commit_ready_v1(actual, references), count <= references.len(),
    ensures stable_released_leases_v1(actual, references, count).map(|_i, entry| read_lease_slot_view(entry))
        == logical::released_leases_v1(model, stable_references_view(references), count),
        stable_released_leases_v1(actual, references, count).len() == actual.leases@.len(),
    decreases count,
{
    if count > 0 {
        stable_released_leases_correspondence(actual, model, references, (count - 1) as nat);
        assert(stable_released_leases_v1(actual, references, count).map(|_i, entry| read_lease_slot_view(entry))
            =~= logical::released_leases_v1(model, stable_references_view(references), count));
    }
}

proof fn stable_release_paired_transition(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1,
    model_before: logical::ReadContentsV1, model_after: logical::ReadContentsV1, consumer: WriterKeyV1,
    references: Seq<ContextReadLeaseReferenceV1>, evidence: WriterKeyV1, observed_free_capacity: usize,
    result: Result<(), ReadErrorV1>, model_result: Result<(), logical::ReadErrorV1>)
    requires stable_represents(before, model_before), stable_guard_storage_v1(before), references.len() <= usize::MAX,
        stable_release_execution_relation_v1(before, after, consumer, references, evidence, observed_free_capacity, result),
        logical::release_execution_relation_v1(model_before, model_after, writer_key_view(consumer),
            stable_references_view(references), writer_key_view(evidence), observed_free_capacity, model_result),
    ensures result == begin_result_from(model_result), stable_represents(after, model_after),
{
    stable_release_decision_correspondence(before, model_before, consumer, references, evidence, observed_free_capacity);
    if result == Ok(()) {
        stable_release_preflight_implies_commit_ready_v1(before, consumer, references, evidence, observed_free_capacity);
        stable_released_requests_correspondence(before, model_before, references);
        stable_released_leases_correspondence(before, model_before, references, references.len());
        assert(Seq::new(references.len(), |i: int| references[i].slot)
            =~= Seq::new(stable_references_view(references).len(), |i: int| stable_references_view(references)[i].slot));
        assert(after.readers@ =~= model_after.readers@) by {
            assert forall|a: int| 0 <= a < after.readers@.len() implies after.readers@[a] == model_after.readers@[a] by {
                assert(stable_released_requests_v1(before, references).take(references.len() as int)
                    =~= stable_released_requests_v1(before, references));
                assert(logical::released_requests_v1(model_before, stable_references_view(references)).take(references.len() as int)
                    =~= logical::released_requests_v1(model_before, stable_references_view(references)));
                stable_slot_count_correspondence(stable_released_requests_v1(before, references), a as usize);
            }
        }
    }
}

// Both executions consume the same explicit observation; neither controls the other's admission.
fn stable_release_historical_exec_v1(actual: &mut ContextReadLeasedJournalV1, model: &mut logical::ReadContentsV1,
    consumer: WriterKeyV1, model_consumer: logical::WriterKeyV1,
    references: &[ContextReadLeaseReferenceV1], model_references: &[logical::ReadReferenceV1],
    evidence: &ContextReadQuiescenceEvidenceV1, model_evidence: logical::WriterKeyV1, observed_free_capacity: usize)
    -> (results: (Result<(), ReadErrorV1>, Result<(), logical::ReadErrorV1>))
    requires stable_represents(*old(actual), *old(model)), logical::reader_invariant_v1(*old(model)),
        writer_key_view(consumer) == model_consumer, stable_references_view(references@) == model_references@,
        writer_key_view(evidence.consumer) == model_evidence,
    ensures results.0 == begin_result_from(results.1), stable_represents(*final(actual), *final(model)),
        logical::reader_invariant_v1(*final(model)),
        stable_release_execution_relation_v1(*old(actual), *final(actual), consumer, references@,
            evidence.consumer, observed_free_capacity, results.0),
        logical::release_execution_relation_v1(*old(model), *final(model), model_consumer, model_references@,
            model_evidence, observed_free_capacity, results.1),
{
    let ghost before = *actual;
    let ghost model_before = *model;
    let _count = references.len();
    let result = actual.release_reads_observed_v1(consumer, references, evidence, observed_free_capacity);
    let model_result = logical::release_verified_v1(model, model_consumer, model_references, model_evidence, observed_free_capacity);
    proof { stable_release_paired_transition(before, *actual, model_before, *model, consumer, references@,
        evidence.consumer, observed_free_capacity, result, model_result); }
    (result, model_result)
}

}

verus! {

fn stable_read_consumer_same_exec_v1(left: WriterKeyV1, right: WriterKeyV1) -> (result: bool)
    ensures result == stable_read_consumer_same_v1(left, right),
{
    retained_writer_key_body!(left, right)
}

fn stable_read_reference_same_exec_v1(left: ContextReadLeaseReferenceV1, right: ContextReadLeaseReferenceV1) -> (result: bool)
    ensures result == stable_read_reference_same_v1(left, right),
{
    stable_read_reference_same_body!(left, right)
}

impl ContextReadLeasedJournalV1 {
    fn lookup_read(&self, reference: ContextReadLeaseReferenceV1) -> (result: Result<ContextAllocationReadV1, ReadErrorV1>)
        ensures result == stable_lease_decision_v1(*self, reference),
    {
        stable_lease_lookup_body!(self, reference, stable_read_reference_same_exec_v1)
    }

    // The production header observes Vec::capacity here; physical refinement remains explicit.
    fn release_reads_observed_v1(&mut self, consumer: WriterKeyV1, references: &[ContextReadLeaseReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1, observed_free_capacity: usize) -> (result: Result<(), ReadErrorV1>)
        requires stable_guard_storage_v1(*old(self)),
        ensures stable_release_execution_relation_v1(*old(self), *final(self), consumer, references@,
            evidence.consumer, observed_free_capacity, result),
    {
        stable_release_execution_body!(verus_exec_expr, self, consumer, references, evidence,
            stable_release_preflight_exec_v1, stable_release_commit_exec_v1,
            [, observed_free_capacity], value, [], [proof { assert(value =~= ()); }],
            [proof { stable_release_preflight_implies_commit_ready_v1(*self, consumer, references@,
                evidence.consumer, observed_free_capacity); }])
    }
}

fn stable_release_order_less_exec_v1(left: StableReadReleaseOrderV1, right: StableReadReleaseOrderV1) -> (result: bool)
    ensures result == stable_release_order_lt_v1(left, right),
{
    stable_release_order_less_body!(left, right)
}

fn stable_release_header_exec_v1(contents: &ContextReadLeasedJournalV1, consumer: WriterKeyV1,
    evidence: WriterKeyV1, count: usize, observed_free_capacity: usize) -> (result: Result<(), ReadErrorV1>)
    ensures result == stable_release_header_v1(*contents, consumer, evidence, count, observed_free_capacity),
{
    stable_release_header_body!(contents, consumer, evidence, count, observed_free_capacity)
}

fn stable_release_item_exec_v1(contents: &ContextReadLeasedJournalV1, consumer: WriterKeyV1,
    reference: ContextReadLeaseReferenceV1, state: StableReadReleaseScanV1) -> (result: Result<StableReadReleaseScanV1, ReadErrorV1>)
    requires stable_guard_storage_v1(*contents), state.group < usize::MAX,
    ensures result == stable_release_item_v1(*contents, consumer, reference, state),
        match result { Ok(next) => next.group <= state.group + 1, Err(_) => true },
{
    stable_release_item_body!(contents, consumer, reference, state)
}

fn stable_release_preflight_exec_v1(contents: &ContextReadLeasedJournalV1, consumer: WriterKeyV1,
    references: &[ContextReadLeaseReferenceV1], evidence: WriterKeyV1, observed_free_capacity: usize) -> (result: Result<(), ReadErrorV1>)
    requires stable_guard_storage_v1(*contents),
    ensures result == stable_release_decision_v1(*contents, consumer, references@, evidence, observed_free_capacity),
{
    stable_release_preflight_body!(verus_exec_expr, contents, consumer, references, evidence,
        [, observed_free_capacity], index, state, [
            invariant index <= references.len(), state.group <= index, stable_guard_storage_v1(*contents),
                stable_release_decision_v1(*contents, consumer, references@, evidence, observed_free_capacity)
                    == stable_release_scan_v1(*contents, consumer, references@, index as nat, state),
            decreases references.len() - index,
        ])
}

fn stable_release_commit_exec_v1(contents: &mut ContextReadLeasedJournalV1, references: &[ContextReadLeaseReferenceV1])
    requires stable_release_commit_ready_v1(*old(contents), references@),
    ensures stable_release_commit_prefix_v1(*old(contents), *final(contents), references@, references@.len()),
{
    stable_release_commit_body!(verus_exec_expr, contents, references, index, reference, entry, allocation, [
        let ghost before = *contents;
        let ghost requests = stable_released_requests_v1(before, references@);
        let reader_capacity = contents.readers.len();
        proof {
            assert(Seq::new(0, |i: int| references@[i].slot) =~= Seq::<usize>::empty());
            assert(before.free_reads@ + Seq::<usize>::empty() =~= before.free_reads@);
            assert(requests.take(0) =~= Seq::<ContextAllocationReadV1>::empty());
        }
    ], [
        invariant index <= references.len(), stable_release_commit_ready_v1(before, references@),
            before.readers@.len() == reader_capacity,
            requests == stable_released_requests_v1(before, references@),
            stable_release_commit_prefix_v1(before, *contents, references@, index as nat),
        decreases references.len() - index,
    ], [
        let ghost previous_readers = contents.readers@;
        proof { stable_released_leases_unselected_v1(before, references@, index as nat, reference.slot as int); }
    ], [
        proof {
            assert(requests.take(index + 1) =~= requests.take(index as int).push(requests[index as int]));
            stable_read_slot_count_push_v1(requests.take(index as int), requests[index as int], allocation);
        }
    ], [
        proof {
            assert forall|a: int| 0 <= a < contents.readers@.len() implies
                contents.readers@[a] == before.readers@[a] - stable_read_slot_count_v1(requests.take(index + 1), a as usize) by {
                assert(previous_readers[a] == before.readers@[a] - stable_read_slot_count_v1(requests.take(index as int), a as usize));
                assert(requests[index as int].allocation.slot == allocation);
                assert(contents.readers@[a] == if a == allocation { previous_readers[a] - 1 } else { previous_readers[a] as int });
                stable_read_slot_count_push_v1(requests.take(index as int), requests[index as int], a as usize);
            }
            assert(contents.free_reads@ =~= before.free_reads@ + Seq::new((index + 1) as nat, |i: int| references@[i].slot));
        }
    ])
}

}

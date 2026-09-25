verus! {

fn producer_release_consumer_same_exec_v1(left: WriterKeyV1, right: WriterKeyV1) -> (result: bool)
    ensures result == producer_release_consumer_same_v1(left, right),
{ retained_writer_key_body!(left, right) }

impl ContextProducerReadJournalV1 {
    // The production header observes Vec::capacity here; physical refinement remains explicit.
    fn release_producer_reads_observed_v1(&mut self, consumer: WriterKeyV1, references: &[ContextProducerReadReferenceV1],
        evidence: &ContextReadQuiescenceEvidenceV1, observed_free_capacity: usize) -> (result: Result<(), ReadErrorV1>)
        requires producer_release_domain_v1(*old(self), consumer, evidence.consumer, references.len(), observed_free_capacity),
        ensures producer_release_execution_relation_v1(*old(self), *final(self), consumer, references@,
            evidence.consumer, observed_free_capacity, result),
    {
        producer_release_execution_body!(verus_exec_expr, self, consumer, references, evidence,
            producer_release_preflight_exec_v1, producer_release_commit_exec_v1,
            [, observed_free_capacity], value, [], [proof { assert(value =~= ()); }],
            [proof { producer_release_preflight_implies_commit_ready_v1(*self, consumer, references@,
                evidence.consumer, observed_free_capacity); }])
    }
}

fn producer_release_order_less_exec_v1(left: ProducerReadReleaseOrderV1, right: ProducerReadReleaseOrderV1) -> (result: bool)
    ensures result == producer_release_order_lt_v1(left, right),
{
    producer_release_order_less_body!(left, right)
}

fn producer_release_header_exec_v1(contents: &ContextProducerReadJournalV1, consumer: WriterKeyV1,
    evidence: WriterKeyV1, count: usize, observed_free_capacity: usize) -> (result: Result<(), ReadErrorV1>)
    ensures result == producer_release_header_v1(*contents, consumer, evidence, count, observed_free_capacity),
{
    producer_release_header_body!(contents, consumer, evidence, count, observed_free_capacity)
}

fn producer_release_item_exec_v1(contents: &ContextProducerReadJournalV1, consumer: WriterKeyV1,
    reference: ContextProducerReadReferenceV1, state: ProducerReadReleaseScanV1) -> (result: Result<ProducerReadReleaseScanV1, ReadErrorV1>)
    requires contents.counts@.len() >= contents.stable.journal.allocations@.len(), state.group < usize::MAX,
    ensures result == producer_release_item_v1(*contents, consumer, reference, state),
        match result { Ok(next) => next.group <= state.group + 1, Err(_) => true },
{
    producer_release_item_body!(contents, consumer, reference, state)
}

fn producer_release_preflight_exec_v1(contents: &ContextProducerReadJournalV1, consumer: WriterKeyV1,
    references: &[ContextProducerReadReferenceV1], evidence: WriterKeyV1, observed_free_capacity: usize) -> (result: Result<(), ReadErrorV1>)
    requires producer_release_domain_v1(*contents, consumer, evidence, references.len(), observed_free_capacity),
    ensures result == producer_release_decision_v1(*contents, consumer, references@, evidence, observed_free_capacity),
{
    producer_release_preflight_body!(verus_exec_expr, contents, consumer, references, evidence,
        [, observed_free_capacity], index, state, [
            invariant index <= references.len(), state.group <= index, contents.counts@.len() >= contents.stable.journal.allocations@.len(),
                producer_release_decision_v1(*contents, consumer, references@, evidence, observed_free_capacity)
                    == producer_release_scan_v1(*contents, consumer, references@, index as nat, state),
            decreases references.len() - index,
        ])
}

fn producer_release_commit_exec_v1(contents: &mut ContextProducerReadJournalV1, references: &[ContextProducerReadReferenceV1])
    requires producer_release_commit_ready_v1(*old(contents), references@),
    ensures producer_release_commit_prefix_v1(*old(contents), *final(contents), references@, references@.len()),
{
    producer_release_commit_body!(verus_exec_expr, contents, references, index, reference, entry, allocation, [
        let ghost before = *contents;
        let ghost requests = producer_released_requests_v1(before, references@);
        let reader_capacity = contents.counts.len();
        proof {
            assert(Seq::new(0, |i: int| references@[i].slot) =~= Seq::<usize>::empty());
            assert(before.free@ + Seq::<usize>::empty() =~= before.free@);
            assert(requests.take(0) =~= Seq::<ContextProducerReadV1>::empty());
        }
    ], [
        invariant index <= references.len(), producer_release_commit_ready_v1(before, references@),
            before.counts@.len() == reader_capacity,
            requests == producer_released_requests_v1(before, references@),
            producer_release_commit_prefix_v1(before, *contents, references@, index as nat),
        decreases references.len() - index,
    ], [
        let ghost previous_readers = contents.counts@;
        proof { producer_released_reservations_unselected_v1(before, references@, index as nat, reference.slot as int); }
    ], [
        proof {
            assert(requests.take(index + 1) =~= requests.take(index as int).push(requests[index as int]));
            producer_request_count_push_v1(requests.take(index as int), requests[index as int], allocation);
        }
    ], [
        proof {
            assert forall|a: int| 0 <= a < contents.counts@.len() implies
                contents.counts@[a] == before.counts@[a] - producer_request_count_v1(requests.take(index + 1), a as usize) by {
                assert(previous_readers[a] == before.counts@[a] - producer_request_count_v1(requests.take(index as int), a as usize));
                assert(requests[index as int].read.allocation.slot == allocation);
                assert(contents.counts@[a] == if a == allocation { previous_readers[a] - 1 } else { previous_readers[a] as int });
                producer_request_count_push_v1(requests.take(index as int), requests[index as int], a as usize);
            }
            assert(contents.free@ =~= before.free@ + Seq::new((index + 1) as nat, |i: int| references@[i].slot));
        }
    ])
}

}

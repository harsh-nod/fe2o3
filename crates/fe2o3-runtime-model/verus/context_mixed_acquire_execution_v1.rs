include!("../src/context_producer_reads/mixed_acquire_bodies.rs");

verus! {

spec fn mixed_acquire_storage_v1(contents: ContextProducerReadJournalV1) -> bool {
    &&& producer_budget_domain_v1(contents)
    &&& stable_guard_storage_v1(contents.stable)
    &&& contents.counts@.len() >= contents.stable.journal.allocations@.len()
}

spec fn mixed_acquire_decision_v1(contents: ContextProducerReadJournalV1, consumer: WriterKeyV1,
    stable: Seq<ContextAllocationReadV1>, stable_output: Seq<Option<ContextReadLeaseReferenceV1>>,
    pending: Seq<ContextProducerReadV1>, producer_output: Seq<Option<ContextProducerReadReferenceV1>>,
) -> Result<(), ReadErrorV1> {
    if consumer.context_generation != contents.stable.journal.context_generation {
        Err(ReadErrorV1::ForeignContext)
    } else if consumer.local == 0 || consumer.local == u64::MAX || consumer.kind != WriterKindV1::Submission {
        Err(ReadErrorV1::InvalidWriterId)
    } else if stable.len() != stable_output.len() || pending.len() != producer_output.len() {
        Err(ReadErrorV1::RosterCapacity)
    } else if stable.len() + pending.len() > usize::MAX
        || stable.len() + pending.len() > producer_remaining_v1(contents) {
        Err(ReadErrorV1::MemberCapacity)
    } else if stable.len() > 0 && stable_acquire_decision_v1(contents.stable, consumer, stable, stable_output).is_err() {
        stable_acquire_decision_v1(contents.stable, consumer, stable, stable_output)
    } else if pending.len() > 0 {
        producer_acquire_decision_v1(contents, consumer, pending, producer_output)
    } else {
        Ok(())
    }
}

spec fn mixed_stable_commit_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    consumer: WriterKeyV1, requests: Seq<ContextAllocationReadV1>,
    output_before: Seq<Option<ContextReadLeaseReferenceV1>>, output_after: Seq<Option<ContextReadLeaseReferenceV1>>,
) -> bool {
    &&& producer_reservations_frame_v1(before, after)
    &&& if requests.len() == 0 { after == before && output_after == output_before }
        else { stable_acquire_commit_relation_v1(before.stable, after.stable, consumer, requests, output_after) }
}

spec fn mixed_producer_commit_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    consumer: WriterKeyV1, requests: Seq<ContextProducerReadV1>,
    output_before: Seq<Option<ContextProducerReadReferenceV1>>, output_after: Seq<Option<ContextProducerReadReferenceV1>>,
) -> bool {
    if requests.len() == 0 { after == before && output_after == output_before }
    else { producer_acquire_commit_relation_v1(before, after, consumer, requests, output_after) }
}

spec fn mixed_acquire_relation_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    consumer: WriterKeyV1, stable: Seq<ContextAllocationReadV1>, pending: Seq<ContextProducerReadV1>,
    stable_before: Seq<Option<ContextReadLeaseReferenceV1>>, stable_after: Seq<Option<ContextReadLeaseReferenceV1>>,
    producer_before: Seq<Option<ContextProducerReadReferenceV1>>, producer_after: Seq<Option<ContextProducerReadReferenceV1>>,
    result: Result<(), ReadErrorV1>,
) -> bool {
    &&& result == mixed_acquire_decision_v1(before, consumer, stable, stable_before, pending, producer_before)
    &&& if result.is_err() {
        after == before && stable_after == stable_before && producer_after == producer_before
    } else {
        &&& producer_budget_domain_v1(after)
        &&& producer_total_retained_v1(after) == producer_total_retained_v1(before) + stable.len() + pending.len()
        &&& exists|middle: ContextProducerReadJournalV1|
            mixed_stable_commit_v1(before, middle, consumer, stable, stable_before, stable_after)
            && mixed_producer_commit_v1(middle, after, consumer, pending, producer_before, producer_after)
    }
}

proof fn mixed_stable_preserves_producer_ready_v1(before: ContextProducerReadJournalV1,
    after: ContextProducerReadJournalV1, requests: Seq<ContextProducerReadV1>)
    requires producer_reservations_frame_v1(before, after), producer_acquire_commit_ready_v1(before, requests),
    ensures producer_acquire_commit_ready_v1(after, requests),
{
    assert forall|i: int| 0 <= i < requests.len() implies
        producer_acquire_prefix_entry_v1(after, requests, i) by {
        assert(producer_acquire_prefix_entry_v1(before, requests, i));
    }
}

proof fn mixed_stable_headroom_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    consumer: WriterKeyV1, stable: Seq<ContextAllocationReadV1>, pending_count: nat,
    output_before: Seq<Option<ContextReadLeaseReferenceV1>>, output_after: Seq<Option<ContextReadLeaseReferenceV1>>)
    requires producer_budget_domain_v1(before), stable.len() <= before.stable.free_reads@.len(),
        stable.len() + pending_count <= producer_remaining_v1(before),
        mixed_stable_commit_v1(before, after, consumer, stable, output_before, output_after),
    ensures producer_budget_domain_v1(after),
        producer_total_retained_v1(after) == producer_total_retained_v1(before) + stable.len(),
        producer_remaining_v1(after) >= pending_count,
        after.stable.journal == before.stable.journal,
{
    if stable.len() > 0 {
        assert(after.stable.free_reads@.len() == before.stable.free_reads@.len() - stable.len());
    }
}

proof fn mixed_producer_headroom_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    consumer: WriterKeyV1, pending: Seq<ContextProducerReadV1>,
    output_before: Seq<Option<ContextProducerReadReferenceV1>>, output_after: Seq<Option<ContextProducerReadReferenceV1>>)
    requires producer_budget_domain_v1(before), pending.len() <= producer_remaining_v1(before),
        mixed_producer_commit_v1(before, after, consumer, pending, output_before, output_after),
    ensures producer_budget_domain_v1(after),
        producer_total_retained_v1(after) == producer_total_retained_v1(before) + pending.len(),
{
    producer_budget_arithmetic_v1(before);
    if pending.len() > 0 {
        assert(after.free@.len() == before.free@.len() - pending.len());
    }
}

impl ContextProducerReadJournalV1 {
    fn acquire_mixed_reads(&mut self, consumer: WriterKeyV1,
        stable_requests: &[ContextAllocationReadV1], stable_output: &mut [Option<ContextReadLeaseReferenceV1>],
        producer_requests: &[ContextProducerReadV1], producer_output: &mut [Option<ContextProducerReadReferenceV1>],
    ) -> (result: Result<(), ReadErrorV1>)
        requires mixed_acquire_storage_v1(*old(self)),
            stable_requests@.len() <= u64::MAX, producer_requests@.len() <= u64::MAX,
        ensures mixed_acquire_relation_v1(*old(self), *final(self), consumer, stable_requests@, producer_requests@,
            old(stable_output)@, final(stable_output)@, old(producer_output)@, final(producer_output)@, result),
    {
        mixed_acquire_execution_body!(verus_exec_expr, self, self.stable.journal.context_generation(), consumer,
            stable_requests, stable_output, producer_requests, producer_output,
            stable_acquire_preflight_exec_v1, stable_acquire_commit_exec_v1,
            producer_acquire_preflight_exec_v1, producer_acquire_commit_exec_v1,
            value,
            [
                let ghost before = *self;
                let ghost stable_before = stable_output@;
                let ghost producer_before = producer_output@;
            ],
            [proof { assert(value =~= ()); }],
            [proof { assert(value =~= ()); }],
            [
                proof {
                    if stable_requests.len() > 0 {
                        stable_acquire_preflight_implies_commit_ready_v1(self.stable, consumer, stable_requests@, stable_output@);
                    }
                    if producer_requests.len() > 0 {
                        producer_acquire_preflight_implies_commit_ready_v1(*self, consumer, producer_requests@, producer_output@);
                    }
                }
            ],
            [
                let ghost middle = *self;
                proof {
                    mixed_stable_headroom_v1(before, *self, consumer, stable_requests@, producer_requests@.len(),
                        stable_before, stable_output@);
                    if producer_requests.len() > 0 {
                        mixed_stable_preserves_producer_ready_v1(before, *self, producer_requests@);
                    }
                }
            ],
            [
                proof {
                    mixed_producer_headroom_v1(middle, *self, consumer, producer_requests@, producer_before, producer_output@);
                    assert(mixed_stable_commit_v1(before, middle, consumer, stable_requests@, stable_before, stable_output@));
                    assert(mixed_producer_commit_v1(middle, *self, consumer, producer_requests@, producer_before, producer_output@));
                }
            ])
    }
}

}

verus! {

impl ContextReadLeasedJournalV1 {
    fn retained_read_count(&self) -> (result: usize)
        requires self.free_reads@.len() <= self.leases@.len(),
        ensures result == self.leases@.len() - self.free_reads@.len(),
    { stable_retained_read_count_body!(self) }
}

impl ContextProducerReadJournalV1 {
    fn retained_producer_read_count(&self) -> (result: usize)
        requires self.free@.len() <= self.reservations@.len(),
        ensures result == producer_retained_v1(*self),
    { producer_retained_count_body!(self) }

    fn retained_read_count(&self) -> (result: usize)
        requires self.free@.len() <= self.reservations@.len(),
            self.stable.free_reads@.len() <= self.stable.leases@.len(),
            producer_total_retained_v1(*self) <= usize::MAX,
        ensures result == producer_total_retained_v1(*self),
    { producer_total_read_count_body!(self) }

    fn remaining_read_slots(&self) -> (result: usize)
        requires producer_budget_domain_v1(*self),
        ensures result == producer_remaining_v1(*self), result <= self.free@.len(),
    {
        proof { producer_budget_arithmetic_v1(*self); }
        producer_remaining_read_slots_body!(self)
    }

    fn validate_read_capacity(&self, count: usize) -> (result: Result<(), ReadErrorV1>)
        requires producer_budget_domain_v1(*self), count <= u64::MAX,
        ensures result == producer_stable_capacity_decision_v1(*self, count),
    { producer_stable_capacity_body!(self, count) }

    fn validate_producer_read_capacity(&self, count: usize) -> (result: Result<(), ReadErrorV1>)
        requires producer_budget_domain_v1(*self), count <= u64::MAX,
        ensures result == producer_capacity_decision_v1(*self, count),
    { producer_capacity_body!(self, count) }

    fn acquire_producer_reads(&mut self, consumer: WriterKeyV1, requests: &[ContextProducerReadV1],
        output: &mut [Option<ContextProducerReadReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
        requires producer_acquire_domain_v1(*old(self), consumer, requests.len(), old(output)@),
            requests@.len() <= u64::MAX,
        ensures producer_acquire_execution_relation_v1(*old(self), *final(self), consumer,
            requests@, old(output)@, final(output)@, result),
    {
        producer_acquire_execution_body!(verus_exec_expr, self, consumer, requests, output,
            producer_acquire_preflight_exec_v1, producer_acquire_commit_exec_v1,
            value, [], [proof { assert(value =~= ()); }],
            [proof {
                producer_budget_arithmetic_v1(*self);
                producer_acquire_preflight_implies_commit_ready_v1(*self, consumer, requests@, output@);
            }])
    }
}

fn producer_read_order_less_exec_v1(left: ProducerReadOrderV1, right: ProducerReadOrderV1) -> (result: bool)
    ensures result == producer_order_lt_v1(left, right),
{
    stable_read_order_less_body!(left, right)
}

fn producer_output_vacant_exec_v1(output: &[Option<ContextProducerReadReferenceV1>]) -> (result: bool)
    ensures result == (forall|i: int| 0 <= i < output@.len() ==> output@[i].is_none()),
{
    stable_read_output_vacant_body!(verus_exec_expr, output, index, [
        invariant index <= output.len(), forall|i: int| 0 <= i < index ==> output@[i].is_none(),
        decreases output.len() - index,
    ])
}

fn producer_acquire_header_exec_v1(contents: &ContextProducerReadJournalV1, consumer: WriterKeyV1,
    count: usize, output: &[Option<ContextProducerReadReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
    requires count <= u64::MAX,
        producer_acquire_prefix_v1(*contents, consumer, count, output@).is_ok() ==> producer_budget_domain_v1(*contents),
    ensures result == producer_acquire_header_v1(*contents, consumer, count, output@),
{
    producer_acquire_header_body!(contents, contents.stable.journal.context_generation(), consumer, count, output)
}

fn producer_acquire_item_exec_v1(contents: &ContextProducerReadJournalV1, consumer: WriterKeyV1, request: ContextProducerReadV1,
    index: usize, state: ProducerReadAcquireScanV1) -> (result: Result<ProducerReadAcquireScanV1, ReadErrorV1>)
    requires contents.counts@.len() >= contents.stable.journal.allocations@.len(), index < contents.free@.len(), state.group < usize::MAX,
    ensures result == producer_acquire_item_v1(*contents, consumer, request, index, state),
        match result { Ok(next) => next.group <= state.group + 1, Err(_) => true },
{
    producer_acquire_item_body!(contents, consumer, request, index, state)
}

fn producer_acquire_preflight_exec_v1(contents: &ContextProducerReadJournalV1, consumer: WriterKeyV1,
    requests: &[ContextProducerReadV1], output: &[Option<ContextProducerReadReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
    requires producer_acquire_domain_v1(*contents, consumer, requests.len(), output@), requests@.len() <= u64::MAX,
    ensures result == producer_acquire_decision_v1(*contents, consumer, requests@, output@),
{
    producer_acquire_preflight_body!(verus_exec_expr, contents, consumer, requests, output, index, state, [
        invariant index <= requests.len(), state.group <= index,
            requests@.len() <= contents.free@.len(), contents.counts@.len() >= contents.stable.journal.allocations@.len(),
            producer_acquire_decision_v1(*contents, consumer, requests@, output@)
                == producer_acquire_scan_v1(*contents, consumer, requests@, index as nat, state),
        decreases requests.len() - index,
    ])
}

fn producer_acquire_commit_exec_v1(contents: &mut ContextProducerReadJournalV1, consumer: WriterKeyV1,
    requests: &[ContextProducerReadV1], output: &mut [Option<ContextProducerReadReferenceV1>])
    requires producer_acquire_commit_ready_v1(*old(contents), requests@), old(output)@.len() == requests@.len(),
    ensures producer_acquire_commit_relation_v1(*old(contents), *final(contents), consumer, requests@, final(output)@),
{
    producer_acquire_commit_body!(verus_exec_expr, contents, consumer, requests, output, index, [
        let ghost before = *contents;
        let ghost output_before = output@;
        let reader_capacity = contents.counts.len();
        proof {
            assert(before.free@.take(before.free@.len() as int) =~= before.free@);
            assert(requests@.take(0) =~= Seq::<ContextProducerReadV1>::empty());
        }
    ], [
        invariant index <= requests.len(), output_before.len() == requests@.len(),
            before.counts@.len() == reader_capacity,
            producer_acquire_commit_ready_v1(before, requests@),
            producer_acquire_commit_prefix_v1(before, *contents, consumer, requests@, output_before, output@, index as nat),
        decreases requests.len() - index,
    ], [
        proof {
            assert(requests@.take(index + 1) =~= requests@.take(index as int).push(requests@[index as int]));
            producer_request_count_push_v1(requests@.take(index as int), requests@[index as int], requests@[index as int].read.allocation.slot);
        }
    ], [
        proof {
            assert forall|a: int| 0 <= a < contents.counts@.len() implies
                contents.counts@[a] == before.counts@[a] + producer_request_count_v1(requests@.take(index + 1), a as usize) by {
                producer_request_count_push_v1(requests@.take(index as int), requests@[index as int], a as usize);
            }
            assert(contents.free@ =~= before.free@.take(before.free@.len() - index - 1));
        }
    ], [proof { assert(requests@.take(requests@.len() as int) =~= requests@); }])
}

}

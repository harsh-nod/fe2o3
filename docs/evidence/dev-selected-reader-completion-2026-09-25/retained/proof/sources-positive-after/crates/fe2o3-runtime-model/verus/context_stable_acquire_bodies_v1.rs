verus! {

impl ContextReadLeasedJournalV1 {
    fn validate_read_capacity(&self, count: usize) -> (result: Result<(), ReadErrorV1>)
        requires count <= u64::MAX,
        ensures result == stable_capacity_decision_v1(*self, count),
    {
        stable_read_capacity_body!(self, count)
    }

    fn validate_read(&self, request: &ContextAllocationReadV1) -> (result: Result<(), ReadErrorV1>)
        ensures result == stable_read_decision_v1(self.journal, *request),
    {
        stable_read_validate_body!(self, request)
    }

    fn acquire_reads(&mut self, consumer: WriterKeyV1, requests: &[ContextAllocationReadV1],
        output: &mut [Option<ContextReadLeaseReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
        requires stable_guard_storage_v1(*old(self)), requests@.len() <= u64::MAX,
        ensures stable_acquire_execution_relation_v1(*old(self), *final(self), consumer,
            requests@, old(output)@, final(output)@, result),
    {
        stable_acquire_execution_body!(verus_exec_expr, self, consumer, requests, output,
            stable_acquire_preflight_exec_v1, stable_acquire_commit_exec_v1,
            value, [], [proof { assert(value =~= ()); }],
            [proof { stable_acquire_preflight_implies_commit_ready_v1(*self, consumer, requests@, output@); }])
    }
}

fn stable_read_order_less_exec_v1(left: StableReadOrderV1, right: StableReadOrderV1) -> (result: bool)
    ensures result == stable_order_lt_v1(left, right),
{
    stable_read_order_less_body!(left, right)
}

fn stable_read_output_vacant_exec_v1(output: &[Option<ContextReadLeaseReferenceV1>]) -> (result: bool)
    ensures result == (forall|i: int| 0 <= i < output@.len() ==> output@[i].is_none()),
{
    stable_read_output_vacant_body!(verus_exec_expr, output, index, [
        invariant index <= output.len(), forall|i: int| 0 <= i < index ==> output@[i].is_none(),
        decreases output.len() - index,
    ])
}

fn stable_acquire_header_exec_v1(contents: &ContextReadLeasedJournalV1, consumer: WriterKeyV1,
    count: usize, output: &[Option<ContextReadLeaseReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
    requires count <= u64::MAX,
    ensures result == stable_acquire_header_v1(*contents, consumer, count, output@),
{
    stable_acquire_header_body!(contents, consumer, count, output)
}

fn stable_acquire_item_exec_v1(contents: &ContextReadLeasedJournalV1, request: ContextAllocationReadV1,
    index: usize, state: StableReadAcquireScanV1) -> (result: Result<StableReadAcquireScanV1, ReadErrorV1>)
    requires stable_guard_storage_v1(*contents), index < contents.free_reads@.len(), state.group < usize::MAX,
    ensures result == stable_acquire_item_v1(*contents, request, index, state),
        match result { Ok(next) => next.group <= state.group + 1, Err(_) => true },
{
    stable_acquire_item_body!(contents, request, index, state)
}

fn stable_acquire_preflight_exec_v1(contents: &ContextReadLeasedJournalV1, consumer: WriterKeyV1,
    requests: &[ContextAllocationReadV1], output: &[Option<ContextReadLeaseReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
    requires stable_guard_storage_v1(*contents), requests@.len() <= u64::MAX,
    ensures result == stable_acquire_decision_v1(*contents, consumer, requests@, output@),
{
    stable_acquire_preflight_body!(verus_exec_expr, contents, consumer, requests, output, index, state, [
        invariant index <= requests.len(), state.group <= index,
            requests@.len() <= contents.free_reads@.len(), stable_guard_storage_v1(*contents),
            stable_acquire_decision_v1(*contents, consumer, requests@, output@)
                == stable_acquire_scan_v1(*contents, requests@, index as nat, state),
        decreases requests.len() - index,
    ])
}

fn stable_acquire_commit_exec_v1(contents: &mut ContextReadLeasedJournalV1, consumer: WriterKeyV1,
    requests: &[ContextAllocationReadV1], output: &mut [Option<ContextReadLeaseReferenceV1>])
    requires stable_acquire_commit_ready_v1(*old(contents), requests@), old(output)@.len() == requests@.len(),
    ensures stable_acquire_commit_relation_v1(*old(contents), *final(contents), consumer, requests@, final(output)@),
{
    stable_acquire_commit_body!(verus_exec_expr, contents, consumer, requests, output, index, [
        let ghost before = *contents;
        let ghost output_before = output@;
        let reader_capacity = contents.readers.len();
        proof {
            assert(before.free_reads@.take(before.free_reads@.len() as int) =~= before.free_reads@);
            assert(requests@.take(0) =~= Seq::<ContextAllocationReadV1>::empty());
        }
    ], [
        invariant index <= requests.len(), output_before.len() == requests@.len(),
            before.readers@.len() == reader_capacity,
            stable_acquire_commit_ready_v1(before, requests@),
            stable_acquire_commit_prefix_v1(before, *contents, consumer, requests@, output_before, output@, index as nat),
        decreases requests.len() - index,
    ], [
        proof {
            assert(requests@.take(index + 1) =~= requests@.take(index as int).push(requests@[index as int]));
            stable_read_slot_count_push_v1(requests@.take(index as int), requests@[index as int], requests@[index as int].allocation.slot);
        }
    ], [
        proof {
            assert forall|a: int| 0 <= a < contents.readers@.len() implies
                contents.readers@[a] == before.readers@[a] + stable_read_slot_count_v1(requests@.take(index + 1), a as usize) by {
                stable_read_slot_count_push_v1(requests@.take(index as int), requests@[index as int], a as usize);
            }
            assert(contents.free_reads@ =~= before.free_reads@.take(before.free_reads@.len() - index - 1));
        }
    ], [proof { assert(requests@.take(requests@.len() as int) =~= requests@); }])
}

}

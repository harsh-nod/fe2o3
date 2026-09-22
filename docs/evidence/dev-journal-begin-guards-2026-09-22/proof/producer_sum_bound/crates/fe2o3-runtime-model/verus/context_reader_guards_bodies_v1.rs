verus! {

impl ContextVersionJournalV1 {
    fn lookup_allocation(&self, reference: AllocationReferenceV1) -> (result: Result<ContextAllocationStateV1, ReadErrorV1>)
        ensures result == allocation_lookup_decision_v1(*self, reference),
    {
        allocation_lookup_body!(self, reference, shared_retained_allocation_v1, begin_indexed_access_v1)
    }

    // The existing one-line journal forwarder is source-bound by qualification.
    fn begin_write(&mut self, writer: WriterReferenceV1, roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
        ensures begin_execution_relation_v1(*old(self), *final(self), writer, roster@, result),
    {
        begin_exec_v1(self, writer, roster)
    }
}

impl ContextReadLeasedJournalV1 {
    fn reader_count(&self, allocation: AllocationReferenceV1) -> (result: Result<usize, ReadErrorV1>)
        requires stable_count_safe_v1(*self, allocation),
        ensures result == stable_count_decision_v1(*self, allocation),
    {
        stable_reader_count_body!(self, allocation)
    }

    fn require_unread_writes(&self, roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
        requires stable_guard_storage_v1(*self),
        ensures result == stable_unread_writes_v1(*self, roster@, 0),
    {
        unread_writes_body!(verus_exec_expr, self, roster, index, [
            invariant index <= roster.len(), stable_guard_storage_v1(*self),
                stable_unread_writes_v1(*self, roster@, 0) == stable_unread_writes_v1(*self, roster@, index as nat),
            decreases roster.len() - index,
        ])
    }

    fn begin_write(&mut self, writer: WriterReferenceV1, roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
        requires stable_guard_storage_v1(*old(self)),
        ensures stable_begin_relation_v1(*old(self), *final(self), writer, roster@, result),
    {
        stable_begin_body!(self, writer, roster)
    }
}

impl ContextProducerReadJournalV1 {
    fn reader_count(&self, allocation: AllocationReferenceV1) -> (result: Result<usize, ReadErrorV1>)
        requires producer_count_safe_v1(*self, allocation),
        ensures result == producer_count_decision_v1(*self, allocation),
    {
        producer_reader_count_body!(self, allocation)
    }

    fn require_unread_writes(&self, roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
        requires producer_guard_storage_v1(*self),
        ensures result == producer_unread_writes_v1(*self, roster@, 0),
    {
        unread_writes_body!(verus_exec_expr, self, roster, index, [
            invariant index <= roster.len(), producer_guard_storage_v1(*self),
                producer_unread_writes_v1(*self, roster@, 0) == producer_unread_writes_v1(*self, roster@, index as nat),
            decreases roster.len() - index,
        ])
    }

    fn begin_write(&mut self, writer: WriterReferenceV1, roster: &[AllocationWriteV1]) -> (result: Result<(), ReadErrorV1>)
        requires producer_guard_storage_v1(*old(self)),
        ensures producer_begin_relation_v1(*old(self), *final(self), writer, roster@, result),
    {
        producer_begin_body!(self, writer, roster)
    }
}

}

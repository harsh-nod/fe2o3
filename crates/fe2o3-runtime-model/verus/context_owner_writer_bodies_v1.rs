verus! {

spec fn owner_writer_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.allocations == before.allocations
    &&& after.allocation_free == before.allocation_free
    &&& after.members == before.members
    &&& after.member_free == before.member_free
    &&& after.scratch == before.scratch
}

spec fn owner_register_decision_v1(before: JournalContentsV1, key: WriterKeyV1)
    -> Result<WriterReferenceV1, ReadErrorV1>
{
    if key.context_generation != before.context_generation { Err(ReadErrorV1::ForeignContext) }
    else if !issuable_id_v1(key.local) { Err(ReadErrorV1::InvalidWriterId) }
    else if key.local <= before.registration_watermark { Err(ReadErrorV1::WriterReplay) }
    else if before.free@.len() == 0 { Err(ReadErrorV1::WriterCapacity) }
    else {
        let slot = before.free@[before.free@.len() - 1];
        if slot >= before.writers@.len() || before.writers@[slot as int].is_some() {
            Err(ReadErrorV1::InvalidState)
        } else if before.reserved_count == usize::MAX || before.reserved_count >= before.writer_capacity {
            Err(ReadErrorV1::InvalidState)
        } else { Ok(WriterReferenceV1 { slot, key }) }
    }
}

spec fn owner_register_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    key: WriterKeyV1, result: Result<WriterReferenceV1, ReadErrorV1>) -> bool
{
    &&& result == owner_register_decision_v1(before, key)
    &&& owner_writer_frame_v1(before, after)
    &&& match result {
        Err(_) => after == before,
        Ok(reference) => {
            &&& after.writers@ == before.writers@.update(reference.slot as int, Some(WriterEntryV1::Reserved(key)))
            &&& after.free@ == before.free@.subrange(0, before.free@.len() - 1)
            &&& after.registration_watermark == key.local
            &&& after.reserved_count == before.reserved_count + 1
        },
    }
}

spec fn owner_abort_decision_v1(before: JournalContentsV1, reference: WriterReferenceV1, capacity: usize)
    -> Result<(), ReadErrorV1>
{
    match begin_reserved_decision_v1(before, reference) {
        Err(error) => Err(error),
        Ok(_) => if before.free@.len() >= before.writer_capacity || before.free@.len() >= capacity
            || before.reserved_count == 0 { Err(ReadErrorV1::InvalidState) } else { Ok(()) },
    }
}

spec fn owner_abort_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    reference: WriterReferenceV1, capacity: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == owner_abort_decision_v1(before, reference, capacity)
    &&& owner_writer_frame_v1(before, after)
    &&& after.registration_watermark == before.registration_watermark
    &&& match result {
        Err(_) => after == before,
        Ok(()) => {
            &&& after.writers@ == before.writers@.update(reference.slot as int, None)
            &&& after.free@ == before.free@.push(reference.slot)
            &&& after.reserved_count == before.reserved_count - 1
        },
    }
}

spec fn owner_writer_stable_frame_v1(before: ContextReadLeasedJournalV1, after: ContextReadLeasedJournalV1) -> bool {
    &&& after.leases == before.leases
    &&& after.free_reads == before.free_reads
    &&& after.readers == before.readers
    &&& after.next_incarnation == before.next_incarnation
}

spec fn owner_writer_producer_frame_v1(before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1) -> bool {
    &&& owner_writer_stable_frame_v1(before.stable, after.stable)
    &&& after.reservations == before.reservations
    &&& after.free == before.free
    &&& after.counts == before.counts
    &&& after.next_incarnation == before.next_incarnation
}

fn owner_writer_issuable_v1(value: u64) -> (result: bool)
    ensures result == issuable_id_v1(value),
{
    value != 0 && value != u64::MAX
}

impl ContextVersionJournalV1 {
    fn register_writer(&mut self, key: WriterKeyV1) -> (result: Result<WriterReferenceV1, ReadErrorV1>)
        ensures owner_register_relation_v1(*old(self), *final(self), key, result),
    {
        writer_register_body!(verus_exec_expr, self, key, owner_writer_issuable_v1, begin_indexed_access_v1)
    }

    fn lookup_reserved(&self, reference: WriterReferenceV1) -> (result: Result<WriterKeyV1, ReadErrorV1>)
        ensures result == begin_reserved_decision_v1(*self, reference),
    {
        writer_reserved_lookup_body!(self, reference, begin_reserved_exec_v1)
    }

    // Vec capacity is an explicit observation, not a physical-allocation proof.
    fn abort_reserved_observed_v1(&mut self, reference: WriterReferenceV1, observed_free_capacity: usize)
        -> (result: Result<(), ReadErrorV1>)
        ensures owner_abort_relation_v1(*old(self), *final(self), reference, observed_free_capacity, result),
    {
        writer_abort_body!(verus_exec_expr, self, reference, observed_free_capacity, begin_indexed_access_v1)
    }
}

impl ContextReadLeasedJournalV1 {
    fn register_writer(&mut self, key: WriterKeyV1) -> (result: Result<WriterReferenceV1, ReadErrorV1>)
        ensures owner_register_relation_v1(old(self).journal, final(self).journal, key, result),
            owner_writer_stable_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        writer_owner_forward_body!(self, journal, register_writer, key, [])
    }

    fn abort_reserved_observed_v1(&mut self, reference: WriterReferenceV1, observed_free_capacity: usize)
        -> (result: Result<(), ReadErrorV1>)
        ensures owner_abort_relation_v1(old(self).journal, final(self).journal, reference, observed_free_capacity, result),
            owner_writer_stable_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        writer_owner_forward_body!(self, journal, abort_reserved_observed_v1, reference, [, observed_free_capacity])
    }
}

impl ContextProducerReadJournalV1 {
    fn register_writer(&mut self, key: WriterKeyV1) -> (result: Result<WriterReferenceV1, ReadErrorV1>)
        ensures owner_register_relation_v1(old(self).stable.journal, final(self).stable.journal, key, result),
            owner_writer_producer_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        writer_owner_forward_body!(self, stable, register_writer, key, [])
    }

    fn abort_reserved_observed_v1(&mut self, reference: WriterReferenceV1, observed_free_capacity: usize)
        -> (result: Result<(), ReadErrorV1>)
        ensures owner_abort_relation_v1(old(self).stable.journal, final(self).stable.journal, reference, observed_free_capacity, result),
            owner_writer_producer_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        writer_owner_forward_body!(self, stable, abort_reserved_observed_v1, reference, [, observed_free_capacity])
    }
}

}

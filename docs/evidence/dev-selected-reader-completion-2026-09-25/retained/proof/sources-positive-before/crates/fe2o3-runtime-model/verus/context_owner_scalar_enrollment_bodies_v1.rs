verus! {

spec fn scalar_enrollment_header_v1(context: u64, key: AllocationKeyV1, device: ContextJournalDeviceKeyV1,
    extent: u64) -> Option<ReadErrorV1>
{
    if key.context_generation != context { Some(ReadErrorV1::ForeignContext) }
    else if !issuable_id_v1(key.local) { Some(ReadErrorV1::InvalidAllocationId) }
    else if device.context_generation != context { Some(ReadErrorV1::ForeignContext) }
    else if !issuable_id_v1(device.local) { Some(ReadErrorV1::InvalidDeviceId) }
    else if extent == 0 { Some(ReadErrorV1::InvalidExtent) }
    else { None }
}

spec fn scalar_enrollment_replay_v1(before: JournalContentsV1, key: AllocationKeyV1) -> bool {
    exists|i: int| 0 <= i < before.allocations@.len()
        && before.allocations@[i].is_some() && before.allocations@[i].unwrap().key == key
}

spec fn scalar_enrollment_decision_v1(before: JournalContentsV1, key: AllocationKeyV1,
    device: ContextJournalDeviceKeyV1, extent: u64) -> Result<AllocationReferenceV1, ReadErrorV1>
{
    if let Some(error) = scalar_enrollment_header_v1(before.context_generation, key, device, extent) { Err(error) }
    else if scalar_enrollment_replay_v1(before, key) { Err(ReadErrorV1::AllocationReplay) }
    else if before.allocation_free@.len() == 0 { Err(ReadErrorV1::AllocationCapacity) }
    else {
        let slot = before.allocation_free@[before.allocation_free@.len() - 1];
        if slot >= before.allocations@.len() || before.allocations@[slot as int].is_some() {
            Err(ReadErrorV1::InvalidState)
        } else { Ok(AllocationReferenceV1 { slot, key }) }
    }
}

spec fn scalar_enrollment_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    key: AllocationKeyV1, device: ContextJournalDeviceKeyV1, extent: u64,
    result: Result<AllocationReferenceV1, ReadErrorV1>) -> bool
{
    &&& result == scalar_enrollment_decision_v1(before, key, device, extent)
    &&& enrollment_untouched_journal_v1(before, after)
    &&& match result {
        Err(_) => after == before,
        Ok(reference) => {
            &&& after.allocations@ == before.allocations@.update(reference.slot as int,
                Some(enrollment_value_v1(EnrollmentV1 { key, device, byte_extent: extent })))
            &&& after.allocation_free@ == before.allocation_free@.subrange(0, before.allocation_free@.len() - 1)
        },
    }
}

impl ContextVersionJournalV1 {
    fn enroll_allocation(&mut self, key: AllocationKeyV1, device: ContextJournalDeviceKeyV1, byte_extent: u64)
        -> (result: Result<AllocationReferenceV1, ReadErrorV1>)
        ensures scalar_enrollment_relation_v1(*old(self), *final(self), key, device, byte_extent, result),
    {
        scalar_enrollment_body!(verus_exec_expr, self, key, device, byte_extent,
            owner_writer_issuable_v1, begin_indexed_access_v1, index, [
                invariant index <= self.allocations.len(), *self == *old(self),
                    scalar_enrollment_header_v1(self.context_generation, key, device, byte_extent).is_none(),
                    forall|i: int| 0 <= i < index && (#[trigger] self.allocations@[i]).is_some()
                        ==> self.allocations@[i].unwrap().key != key,
                decreases self.allocations.len() - index,
            ])
    }
}

impl ContextReadLeasedJournalV1 {
    fn enroll_allocation(&mut self, key: AllocationKeyV1, device: ContextJournalDeviceKeyV1, extent: u64)
        -> (result: Result<AllocationReferenceV1, ReadErrorV1>)
        ensures scalar_enrollment_relation_v1(old(self).journal, final(self).journal, key, device, extent, result),
            owner_writer_stable_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        scalar_enrollment_forward_body!(self, journal, key, device, extent)
    }
}

impl ContextProducerReadJournalV1 {
    fn enroll_allocation(&mut self, key: AllocationKeyV1, device: ContextJournalDeviceKeyV1, extent: u64)
        -> (result: Result<AllocationReferenceV1, ReadErrorV1>)
        ensures scalar_enrollment_relation_v1(old(self).stable.journal, final(self).stable.journal, key, device, extent, result),
            owner_writer_producer_frame_v1(*old(self), *final(self)),
            result.is_err() ==> *final(self) == *old(self),
    {
        scalar_enrollment_forward_body!(self, stable, key, device, extent)
    }
}

}

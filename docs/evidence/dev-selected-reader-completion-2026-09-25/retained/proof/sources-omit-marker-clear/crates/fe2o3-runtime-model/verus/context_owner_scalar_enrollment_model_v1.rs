// New independent scalar model, not a projection of an older scalar executor.
use super::*;

verus! {

pub open spec fn scalar_enrollment_header_v1(context: u64, entry: EnrollmentV1) -> Option<EnrollmentErrorV1> {
    if entry.key.context_generation != context { Some(EnrollmentErrorV1::ForeignContext) }
    else if !issuable_id_v1(entry.key.local) { Some(EnrollmentErrorV1::InvalidAllocationId) }
    else if entry.device.context_generation != context { Some(EnrollmentErrorV1::ForeignContext) }
    else if !issuable_id_v1(entry.device.local) { Some(EnrollmentErrorV1::InvalidDeviceId) }
    else if entry.byte_extent == 0 { Some(EnrollmentErrorV1::InvalidExtent) }
    else { None }
}

pub open spec fn scalar_enrollment_replay_v1(before: JournalContentsV1, key: AllocationKeyV1) -> bool {
    exists|i: int| 0 <= i < before.allocations@.len()
        && before.allocations@[i].is_some() && before.allocations@[i].unwrap().key == key
}

pub open spec fn scalar_enrollment_decision_v1(before: JournalContentsV1, entry: EnrollmentV1)
    -> Result<AllocationReferenceV1, EnrollmentErrorV1>
{
    if let Some(error) = scalar_enrollment_header_v1(before.context_generation, entry) { Err(error) }
    else if scalar_enrollment_replay_v1(before, entry.key) { Err(EnrollmentErrorV1::AllocationReplay) }
    else if before.allocation_free@.len() == 0 { Err(EnrollmentErrorV1::AllocationCapacity) }
    else {
        let slot = before.allocation_free@[before.allocation_free@.len() - 1];
        if slot >= before.allocations@.len() || before.allocations@[slot as int].is_some() {
            Err(EnrollmentErrorV1::InvalidState)
        } else { Ok(AllocationReferenceV1 { slot, key: entry.key }) }
    }
}

pub open spec fn scalar_enrollment_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    entry: EnrollmentV1, result: Result<AllocationReferenceV1, EnrollmentErrorV1>) -> bool
{
    &&& result == scalar_enrollment_decision_v1(before, entry)
    &&& enrollment_untouched_journal_v1(before, after)
    &&& after.writers == before.writers && after.free == before.free
    &&& after.members == before.members && after.member_free == before.member_free && after.scratch == before.scratch
    &&& match result {
        Err(_) => after == before,
        Ok(reference) => {
            &&& after.allocations@ == before.allocations@.update(reference.slot as int, Some(enrollment_value_v1(entry)))
            &&& after.allocation_free@ == before.allocation_free@.subrange(0, before.allocation_free@.len() - 1)
        },
    }
}

pub fn scalar_enrollment_exec_v1(journal: &mut JournalContentsV1, entry: EnrollmentV1)
    -> (result: Result<AllocationReferenceV1, EnrollmentErrorV1>)
    ensures scalar_enrollment_relation_v1(*old(journal), *final(journal), entry, result),
{
    if entry.key.context_generation != journal.context_generation { return Err(EnrollmentErrorV1::ForeignContext); }
    if !issuable_id_exec_v1(entry.key.local) { return Err(EnrollmentErrorV1::InvalidAllocationId); }
    if entry.device.context_generation != journal.context_generation { return Err(EnrollmentErrorV1::ForeignContext); }
    if !issuable_id_exec_v1(entry.device.local) { return Err(EnrollmentErrorV1::InvalidDeviceId); }
    if entry.byte_extent == 0 { return Err(EnrollmentErrorV1::InvalidExtent); }
    let mut index = 0usize;
    while index < journal.allocations.len()
        invariant index <= journal.allocations.len(), *journal == *old(journal),
            scalar_enrollment_header_v1(journal.context_generation, entry).is_none(),
            forall|i: int| 0 <= i < index && (#[trigger] journal.allocations@[i]).is_some()
                ==> journal.allocations@[i].unwrap().key != entry.key,
        decreases journal.allocations.len() - index,
    {
        if let Some(stored) = journal.allocations[index] {
            if stored.key.context_generation == entry.key.context_generation && stored.key.local == entry.key.local {
                return Err(EnrollmentErrorV1::AllocationReplay);
            }
        }
        index += 1;
    }
    if journal.allocation_free.len() == 0 { return Err(EnrollmentErrorV1::AllocationCapacity); }
    let slot = journal.allocation_free[journal.allocation_free.len() - 1];
    if slot >= journal.allocations.len() || journal.allocations[slot].is_some() {
        return Err(EnrollmentErrorV1::InvalidState);
    }
    let _ = journal.allocation_free.pop();
    journal.allocations.set(slot, Some(AllocationEntryV1 {
        key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
        attempt_epoch: 0, content_lineage: 0, pending_member: None,
    }));
    Ok(AllocationReferenceV1 { slot, key: entry.key })
}

// Only a successful transition is specialized to the existing singleton commit
// relation. Admission remains scalar and is not replaced by batch admission.
pub proof fn scalar_enrollment_singleton_v1(before: ReadContentsV1, after: ReadContentsV1,
    entry: EnrollmentV1, reference: AllocationReferenceV1)
    requires scalar_enrollment_relation_v1(before.journal, after.journal, entry, Ok(reference)),
    ensures enrollment_fresh_v1(before.journal, seq![entry]),
        enrollment_final_relation_v1(before, after, seq![entry], seq![Some(reference)],
            (before.journal.allocation_free@.len() - 1) as usize),
{
    vstd::std_specs::vec::axiom_spec_len(&before.journal.allocation_free);
    reveal_with_fuel(enrollment_prefix_v1, 2);
    assert(enrollment_fresh_v1(before.journal, seq![entry]));
    assert(enrollment_prefix_v1(before.journal.allocations@, seq![entry], seq![Some(reference)], 1)
        == before.journal.allocations@.update(reference.slot as int, Some(enrollment_value_v1(entry))));
}

pub proof fn scalar_enrollment_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    entry: EnrollmentV1, result: Result<AllocationReferenceV1, EnrollmentErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires producer_storage_frame_v1(before, after),
        scalar_enrollment_relation_v1(before.stable.journal, after.stable.journal, entry, result),
    ensures producer_invariant_v1(before) ==> producer_invariant_v1(after),
        issued_producer_v1(before, storage, history) ==> issued_producer_v1(after, storage, history),
        forall|request: ProducerReadV1| producer_status_v1(before.stable.journal, request).is_some()
            ==> #[trigger] producer_status_v1(after.stable.journal, request) == producer_status_v1(before.stable.journal, request),
{
    if let Ok(reference) = result {
        scalar_enrollment_singleton_v1(before.stable, after.stable, entry, reference);
        let entries = seq![entry];
        let output = seq![Some(reference)];
        let remaining = (before.stable.journal.allocation_free@.len() - 1) as usize;
        enrollment_final_entries_v1(before.stable, after.stable, entries, output, remaining);
        if issued_producer_v1(before, storage, history) {
            enrollment_preserves_issued_producer_v1(before, after, entries, output, remaining, storage, history);
        }
        if producer_invariant_v1(before) {
            enrollment_pending_custody_preserves_v1(before.stable, after.stable, entries, output, remaining);
            enrollment_prefix_reader_frame_v1(before.stable, entries, output, remaining, 1);
            reader_allocation_frame_preserves_v1(before.stable, after.stable);
            assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some()
                implies producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
                let retained = before.reservations@[s].unwrap();
                assert(producer_entry_valid_v1(before.stable.journal, retained, s, before.next_incarnation));
                reveal(producer_entry_valid_v1);
                producer_status_enrollment_frame_v1(before.stable.journal, after.stable.journal, retained.request);
            }
        }
        assert forall|request: ProducerReadV1| producer_status_v1(before.stable.journal, request).is_some()
            implies #[trigger] producer_status_v1(after.stable.journal, request) == producer_status_v1(before.stable.journal, request) by {
            producer_status_enrollment_frame_v1(before.stable.journal, after.stable.journal, request);
        }
    }
}

}

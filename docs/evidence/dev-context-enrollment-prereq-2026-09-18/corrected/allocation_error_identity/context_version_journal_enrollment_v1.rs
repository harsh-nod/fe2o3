// Enrollment prerequisites: exact admission prefix and conditional commit suffix.
// The production sorting/search middle phase is not refined by this packet.
include!("context_read_invariant_v1.rs");

verus! {

#[derive(Clone, Copy)]
pub struct EnrollmentV1 {
    pub key: AllocationKeyV1,
    pub device: DeviceKeyV1,
    pub byte_extent: u64,
}

#[derive(Clone, Copy)]
pub enum EnrollmentErrorV1 {
    RosterCapacity, InvalidState, ForeignContext, InvalidAllocationId,
    InvalidDeviceId, InvalidExtent, NonCanonicalRoster,
}

pub open spec fn enrollment_key_less_v1(left: AllocationKeyV1, right: AllocationKeyV1) -> bool {
    left.context_generation < right.context_generation
        || (left.context_generation == right.context_generation && left.local < right.local)
}

pub fn enrollment_key_less_exec_v1(left: AllocationKeyV1, right: AllocationKeyV1) -> (result: bool)
    ensures result == enrollment_key_less_v1(left, right),
{
    left.context_generation < right.context_generation
        || (left.context_generation == right.context_generation && left.local < right.local)
}

pub open spec fn enrollment_entry_error_v1(context: u64, entry: EnrollmentV1) -> Option<EnrollmentErrorV1> {
    if entry.key.context_generation != context || entry.device.context_generation != context {
        Some(EnrollmentErrorV1::ForeignContext)
    } else if !issuable_id_v1(entry.key.local) {
        Some(EnrollmentErrorV1::InvalidAllocationId)
    } else if !issuable_id_v1(entry.device.local) {
        Some(EnrollmentErrorV1::InvalidDeviceId)
    } else if entry.byte_extent == 0 {
        Some(EnrollmentErrorV1::InvalidExtent)
    } else { None }
}

pub fn enrollment_entry_error_exec_v1(context: u64, entry: EnrollmentV1) -> (result: Option<EnrollmentErrorV1>)
    ensures result == enrollment_entry_error_v1(context, entry),
{
    if entry.key.context_generation != context || entry.device.context_generation != context {
        Some(EnrollmentErrorV1::ForeignContext)
    } else if !issuable_id_exec_v1(entry.key.local) {
        Some(EnrollmentErrorV1::InvalidDeviceId)
    } else if !issuable_id_exec_v1(entry.device.local) {
        Some(EnrollmentErrorV1::InvalidDeviceId)
    } else if entry.byte_extent == 0 {
        Some(EnrollmentErrorV1::InvalidExtent)
    } else { None }
}

pub open spec fn enrollment_roster_scan_v1(context: u64, entries: Seq<EnrollmentV1>, index: nat)
    -> Result<(), EnrollmentErrorV1>
    decreases entries.len() - index,
{
    if index >= entries.len() { Ok(()) }
    else if let Some(error) = enrollment_entry_error_v1(context, entries[index as int]) { Err(error) }
    else if index > 0 && !enrollment_key_less_v1(entries[index - 1].key, entries[index as int].key) {
        Err(EnrollmentErrorV1::NonCanonicalRoster)
    } else { enrollment_roster_scan_v1(context, entries, index + 1) }
}

pub open spec fn enrollment_header_decision_v1(context: u64, capacity: usize, free_len: usize,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>) -> Result<(), EnrollmentErrorV1>
{
    if entries.len() > capacity || output.len() != entries.len() {
        Err(EnrollmentErrorV1::RosterCapacity)
    } else if exists|i: int| 0 <= i < output.len() && output[i].is_some() {
        Err(EnrollmentErrorV1::InvalidState)
    } else { match enrollment_roster_scan_v1(context, entries, 0) {
        Err(error) => Err(error),
        Ok(()) => if entries.len() > 0 && free_len > capacity {
            Err(EnrollmentErrorV1::InvalidState)
        } else { Ok(()) },
    } }
}

// Matches allocation_lifecycle.rs through the free-length guard, before replay search.
pub fn enrollment_header_exec_v1(context: u64, capacity: usize, free_len: usize,
    entries: &[EnrollmentV1], output: &mut [Option<AllocationReferenceV1>])
    -> (result: Result<(), EnrollmentErrorV1>)
    ensures result == enrollment_header_decision_v1(context, capacity, free_len, entries@, old(output)@),
        final(output)@ == old(output)@,
{
    if entries.len() > capacity || output.len() != entries.len() {
        return Err(EnrollmentErrorV1::RosterCapacity);
    }
    let mut index = 0usize;
    while index < output.len()
        invariant index <= output.len(), output@ == old(output)@,
            entries.len() <= capacity, output.len() == entries.len(),
            forall|i: int| 0 <= i < index ==> output@[i].is_none(),
        decreases output.len() - index,
    {
        if output[index].is_some() { return Err(EnrollmentErrorV1::InvalidState); }
        index += 1;
    }
    let mut index = 0usize;
    while index < entries.len()
        invariant index <= entries.len(), output@ == old(output)@,
            output.len() == entries.len(), entries.len() <= capacity,
            forall|i: int| 0 <= i < output.len() ==> output@[i].is_none(),
            enrollment_roster_scan_v1(context, entries@, 0)
                == enrollment_roster_scan_v1(context, entries@, index as nat),
        decreases entries.len() - index,
    {
        if let Some(error) = enrollment_entry_error_exec_v1(context, entries[index]) { return Err(error); }
        if index > 0 && !enrollment_key_less_exec_v1(entries[index - 1].key, entries[index].key) {
            return Err(EnrollmentErrorV1::NonCanonicalRoster);
        }
        index += 1;
    }
    if entries.len() == 0 { return Ok(()); }
    if free_len > capacity { return Err(EnrollmentErrorV1::InvalidState); }
    Ok(())
}

pub proof fn vacant_allocation_has_no_readers_v1(contents: ReadContentsV1, slot: usize)
    requires reader_invariant_v1(contents), slot < contents.journal.allocations@.len(),
        contents.journal.allocations@[slot as int].is_none(),
    ensures contents.readers@[slot as int] == 0,
{
    reader_count_consequences_v1(contents, slot);
}

pub open spec fn enrollment_value_v1(entry: EnrollmentV1) -> AllocationEntryV1 {
    AllocationEntryV1 { key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
        attempt_epoch: 0, content_lineage: 0, pending_member: None }
}

pub open spec fn enrollment_output_bound_v1(before: ReadContentsV1, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>, remaining: usize) -> bool {
    &&& entries.len() == output.len()
    &&& remaining <= before.journal.allocation_free@.len()
    &&& before.journal.allocation_free@.len() - remaining == entries.len()
    &&& forall|i: int| 0 <= i < entries.len() ==> {
        &&& output[i].is_some()
        &&& output[i].unwrap().key == entries[i].key
        &&& output[i].unwrap().slot == before.journal.allocation_free@[before.journal.allocation_free@.len() - 1 - i]
        &&& output[i].unwrap().slot < before.journal.allocations@.len()
        &&& before.journal.allocations@[output[i].unwrap().slot as int].is_none()
    }
    &&& forall|i: int, j: int| 0 <= i < j < entries.len()
        ==> output[i].unwrap().slot != output[j].unwrap().slot
}

pub open spec fn enrollment_prefix_v1(allocations: Seq<Option<AllocationEntryV1>>, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>, count: nat) -> Seq<Option<AllocationEntryV1>>
    recommends count <= entries.len(), entries.len() == output.len(),
    decreases count,
{
    if count == 0 { allocations }
    else { enrollment_prefix_v1(allocations, entries, output, (count - 1) as nat)
        .update(output[count - 1].unwrap().slot as int, Some(enrollment_value_v1(entries[count - 1]))) }
}

pub proof fn enrollment_prefix_reader_frame_v1(before: ReadContentsV1, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>, remaining: usize, count: nat)
    requires reader_invariant_v1(before), enrollment_output_bound_v1(before, entries, output, remaining),
        count <= entries.len(),
    ensures enrollment_prefix_v1(before.journal.allocations@, entries, output, count).len()
            == before.journal.allocations@.len(),
        forall|a: int| 0 <= a < before.readers@.len() && before.readers@[a] > 0 ==>
            enrollment_prefix_v1(before.journal.allocations@, entries, output, count)[a]
                == before.journal.allocations@[a],
    decreases count,
{
    if count > 0 {
        enrollment_prefix_reader_frame_v1(before, entries, output, remaining, (count - 1) as nat);
        let slot = output[count - 1].unwrap().slot;
        vacant_allocation_has_no_readers_v1(before, slot);
    }
}

pub open spec fn enrollment_untouched_journal_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.reserved_count == before.reserved_count
    &&& after.writers@ == before.writers@
    &&& after.free@ == before.free@
    &&& after.members@ == before.members@
    &&& after.member_free@ == before.member_free@
    &&& after.scratch@ == before.scratch@
}

// This is the real final write/truncate loop under explicit middle-phase obligations.
// It does not assert that Rust sorting/search or the complete caller establishes them.
pub fn enrollment_commit_suffix_exec_v1(contents: &mut ReadContentsV1, entries: &[EnrollmentV1],
    output: &[Option<AllocationReferenceV1>], remaining: usize)
    requires reader_invariant_v1(*old(contents)),
        enrollment_output_bound_v1(*old(contents), entries@, output@, remaining),
    ensures reader_invariant_v1(*final(contents)), reader_storage_frame_v1(*old(contents), *final(contents)),
        enrollment_untouched_journal_v1(old(contents).journal, final(contents).journal),
        final(contents).journal.allocations@ == enrollment_prefix_v1(old(contents).journal.allocations@, entries@, output@, entries@.len()),
        final(contents).journal.allocation_free@ == old(contents).journal.allocation_free@.subrange(0, remaining as int),
{
    let ghost before = *contents;
    let mut index = 0usize;
    while index < entries.len()
        invariant index <= entries.len(), reader_invariant_v1(before),
            enrollment_output_bound_v1(before, entries@, output@, remaining),
            reader_storage_frame_v1(before, *contents),
            enrollment_untouched_journal_v1(before.journal, contents.journal),
            contents.journal.allocation_free@ == before.journal.allocation_free@,
            contents.journal.allocations@ == enrollment_prefix_v1(before.journal.allocations@, entries@, output@, index as nat),
            contents.journal.allocations@.len() == before.journal.allocations@.len(),
        decreases entries.len() - index,
    {
        let entry = entries[index];
        let reference = output[index].unwrap();
        contents.journal.allocations.set(reference.slot, Some(AllocationEntryV1 {
            key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
            attempt_epoch: 0, content_lineage: 0, pending_member: None,
        }));
        index += 1;
    }
    contents.journal.allocation_free.truncate(remaining);
    proof {
        enrollment_prefix_reader_frame_v1(before, entries@, output@, remaining, entries@.len());
        reader_allocation_frame_preserves_v1(before, *contents);
    }
}

}

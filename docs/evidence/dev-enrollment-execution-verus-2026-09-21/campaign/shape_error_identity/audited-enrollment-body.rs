use vstd::prelude::*;

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
    InvalidDeviceId, InvalidExtent, NonCanonicalRoster, AllocationReplay, AllocationCapacity,
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
        Some(EnrollmentErrorV1::InvalidAllocationId)
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
        return Err(EnrollmentErrorV1::InvalidState);
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
    enrollment_journal_output_bound_v1(before.journal, entries, output, remaining)
}

pub open spec fn enrollment_journal_output_bound_v1(before: JournalContentsV1, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>, remaining: usize) -> bool {
    &&& entries.len() == output.len()
    &&& remaining <= before.allocation_free@.len()
    &&& before.allocation_free@.len() - remaining == entries.len()
    &&& forall|i: int| 0 <= i < entries.len() ==> {
        &&& output[i].is_some()
        &&& output[i].unwrap().key == entries[i].key
        &&& output[i].unwrap().slot == before.allocation_free@[before.allocation_free@.len() - 1 - i]
        &&& output[i].unwrap().slot < before.allocations@.len()
        &&& before.allocations@[output[i].unwrap().slot as int].is_none()
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

pub fn enrollment_commit_journal_exec_v1(contents: &mut JournalContentsV1, entries: &[EnrollmentV1],
    output: &[Option<AllocationReferenceV1>], remaining: usize)
    requires enrollment_journal_output_bound_v1(*old(contents), entries@, output@, remaining),
    ensures enrollment_untouched_journal_v1(*old(contents), *final(contents)),
        final(contents).allocations@ == enrollment_prefix_v1(old(contents).allocations@, entries@, output@, entries@.len()),
        final(contents).allocation_free@ == old(contents).allocation_free@.subrange(0, remaining as int),
{
    let ghost before = *contents;
    let mut index = 0usize;
    while index < entries.len()
        invariant index <= entries.len(), before == *old(contents),
            enrollment_journal_output_bound_v1(before, entries@, output@, remaining),
            enrollment_untouched_journal_v1(before, *contents),
            contents.allocation_free@ == before.allocation_free@,
            contents.allocations@ == enrollment_prefix_v1(before.allocations@, entries@, output@, index as nat),
            contents.allocations@.len() == before.allocations@.len(),
        decreases entries.len() - index,
    {
        let entry = entries[index];
        let reference = output[index].unwrap();
        contents.allocations.set(reference.slot, Some(AllocationEntryV1 {
            key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
            attempt_epoch: 0, content_lineage: 0, pending_member: None,
        }));
        index += 1;
    }
    contents.allocation_free.truncate(remaining);
}

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
    enrollment_commit_journal_exec_v1(&mut contents.journal, entries, output, remaining);
    proof {
        enrollment_prefix_reader_frame_v1(before, entries@, output@, remaining, entries@.len());
        reader_allocation_frame_preserves_v1(before, *contents);
    }
}

pub open spec fn enrollment_fresh_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>) -> bool {
    &&& forall|i: int| 0 <= i < entries.len()
        ==> enrollment_entry_error_v1(journal.context_generation, #[trigger] entries[i]).is_none()
    &&& forall|i: int, j: int| 0 <= i < j < entries.len()
        ==> (#[trigger] entries[i]).key != (#[trigger] entries[j]).key
    &&& forall|a: int, i: int| 0 <= a < journal.allocations@.len() && 0 <= i < entries.len()
        && (#[trigger] journal.allocations@[a]).is_some()
        ==> journal.allocations@[a].unwrap().key != (#[trigger] entries[i]).key
}

pub proof fn enrollment_prefix_selected_v1(before: ReadContentsV1, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>, remaining: usize, count: nat, i: int)
    requires enrollment_output_bound_v1(before, entries, output, remaining),
        0 <= i < count <= entries.len(),
    ensures enrollment_prefix_v1(before.journal.allocations@, entries, output, count).len()
            == before.journal.allocations@.len(),
        enrollment_prefix_v1(before.journal.allocations@, entries, output, count)[output[i].unwrap().slot as int]
            == Some(enrollment_value_v1(entries[i])),
    decreases count,
{
    if i < count - 1 {
        enrollment_prefix_selected_v1(before, entries, output, remaining, (count - 1) as nat, i);
    } else {
        assert(count > 0);
        assert(enrollment_prefix_v1(before.journal.allocations@, entries, output, (count - 1) as nat).len()
            == before.journal.allocations@.len()) by {
            enrollment_prefix_length_v1(before, entries, output, remaining, (count - 1) as nat);
        }
    }
}

pub proof fn enrollment_prefix_length_v1(before: ReadContentsV1, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>, remaining: usize, count: nat)
    requires enrollment_output_bound_v1(before, entries, output, remaining), count <= entries.len(),
    ensures enrollment_prefix_v1(before.journal.allocations@, entries, output, count).len()
        == before.journal.allocations@.len(),
    decreases count,
{
    if count > 0 {
        enrollment_prefix_length_v1(before, entries, output, remaining, (count - 1) as nat);
    }
}

pub proof fn enrollment_prefix_unselected_v1(before: ReadContentsV1, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>, remaining: usize, count: nat, a: int)
    requires enrollment_output_bound_v1(before, entries, output, remaining), count <= entries.len(),
        0 <= a < before.journal.allocations@.len(),
        forall|i: int| 0 <= i < count ==> (#[trigger] output[i]).unwrap().slot != a,
    ensures enrollment_prefix_v1(before.journal.allocations@, entries, output, count)[a]
        == before.journal.allocations@[a],
    decreases count,
{
    if count > 0 {
        enrollment_prefix_unselected_v1(before, entries, output, remaining, (count - 1) as nat, a);
    }
}

pub open spec fn enrollment_final_relation_v1(before: ReadContentsV1, after: ReadContentsV1,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>, remaining: usize) -> bool {
    &&& enrollment_output_bound_v1(before, entries, output, remaining)
    &&& enrollment_untouched_journal_v1(before.journal, after.journal)
    &&& after.journal.allocations@ == enrollment_prefix_v1(before.journal.allocations@, entries, output, entries.len())
    &&& after.journal.allocation_free@ == before.journal.allocation_free@.subrange(0, remaining as int)
}

pub proof fn enrollment_final_entries_v1(before: ReadContentsV1, after: ReadContentsV1,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>, remaining: usize)
    requires enrollment_final_relation_v1(before, after, entries, output, remaining),
    ensures after.journal.allocations@.len() == before.journal.allocations@.len(),
        forall|a: int| 0 <= a < before.journal.allocations@.len() && (#[trigger] before.journal.allocations@[a]).is_some()
            ==> after.journal.allocations@[a] == before.journal.allocations@[a],
        forall|a: int| 0 <= a < after.journal.allocations@.len() && (#[trigger] after.journal.allocations@[a]).is_some()
            ==> before.journal.allocations@[a].is_some()
                || exists|i: int| 0 <= i < entries.len() && output[i].unwrap().slot == a
                    && after.journal.allocations@[a] == Some(enrollment_value_v1(entries[i])),
{
    enrollment_prefix_length_v1(before, entries, output, remaining, entries.len());
    assert forall|a: int| 0 <= a < before.journal.allocations@.len() && (#[trigger] before.journal.allocations@[a]).is_some()
        implies after.journal.allocations@[a] == before.journal.allocations@[a] by {
        assert forall|i: int| 0 <= i < entries.len() implies (#[trigger] output[i]).unwrap().slot != a by {
            assert(before.journal.allocations@[output[i].unwrap().slot as int].is_none());
        }
        enrollment_prefix_unselected_v1(before, entries, output, remaining, entries.len(), a);
    }
    assert forall|a: int| 0 <= a < after.journal.allocations@.len() && (#[trigger] after.journal.allocations@[a]).is_some()
        implies before.journal.allocations@[a].is_some()
            || exists|i: int| 0 <= i < entries.len() && output[i].unwrap().slot == a
                && after.journal.allocations@[a] == Some(enrollment_value_v1(entries[i])) by {
        if exists|i: int| 0 <= i < entries.len() && output[i].unwrap().slot == a {
            let i = choose|i: int| 0 <= i < entries.len() && output[i].unwrap().slot == a;
            enrollment_prefix_selected_v1(before, entries, output, remaining, entries.len(), i);
        } else {
            enrollment_prefix_unselected_v1(before, entries, output, remaining, entries.len(), a);
        }
    }
}

#[verifier::spinoff_prover]
pub proof fn enrollment_suffix_partition_v1(before: ReadContentsV1, after: ReadContentsV1,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>, remaining: usize)
    requires slot_partition_v1(before.journal.allocations@, before.journal.allocation_free@),
        enrollment_final_relation_v1(before, after, entries, output, remaining),
        before.journal.allocations@.len() <= usize::MAX,
    ensures slot_partition_v1(after.journal.allocations@, after.journal.allocation_free@),
{
    let free = before.journal.allocation_free@;
    let kept = after.journal.allocation_free@;
    enrollment_prefix_length_v1(before, entries, output, remaining, entries.len());
    assert(kept.no_duplicates());
    assert forall|a: int| 0 <= a < after.journal.allocations@.len() implies
        (#[trigger] after.journal.allocations@[a]).is_none() == kept.contains(a as usize) by {
        if kept.contains(a as usize) {
            let p = choose|p: int| 0 <= p < kept.len() && kept[p] == a;
            assert(free[p] == a);
            assert(free.contains(a as usize));
            assert forall|i: int| 0 <= i < entries.len() implies (#[trigger] output[i]).unwrap().slot != a by {
                let q = free.len() - 1 - i;
                assert(remaining <= q < free.len());
                assert(free[p] != free[q]);
            }
            enrollment_prefix_unselected_v1(before, entries, output, remaining, entries.len(), a);
        } else if after.journal.allocations@[a].is_none() {
            if exists|i: int| 0 <= i < entries.len() && output[i].unwrap().slot == a {
                let i = choose|i: int| 0 <= i < entries.len() && output[i].unwrap().slot == a;
                enrollment_prefix_selected_v1(before, entries, output, remaining, entries.len(), i);
            } else {
                enrollment_prefix_unselected_v1(before, entries, output, remaining, entries.len(), a);
                assert(free.contains(a as usize));
                let p = choose|p: int| 0 <= p < free.len() && free[p] == a;
                if p < remaining {
                    assert(kept[p] == a);
                    assert(kept.contains(a as usize));
                } else {
                    let i = free.len() - 1 - p;
                    assert(0 <= i < entries.len());
                    assert(output[i].unwrap().slot == a);
                }
            }
        }
    }
}

pub open spec fn occupied_allocation_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& enrollment_untouched_journal_v1(before, after)
    &&& after.allocations@.len() == before.allocations@.len()
    &&& forall|a: int| 0 <= a < before.allocations@.len() && (#[trigger] before.allocations@[a]).is_some()
        ==> after.allocations@[a] == before.allocations@[a]
}

pub proof fn retained_chain_enrollment_frame_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>)
    requires enrollment_untouched_journal_v1(before, after), retained_chain_v1(before, writer, chain),
    ensures retained_chain_v1(after, writer, chain),
{
    assert forall|i: int| 0 <= i < chain.len() implies #[trigger] chain_link_v1(after, writer, chain, i) by {
        assert(chain_link_v1(before, writer, chain, i));
        reveal(chain_link_v1);
    }
}

pub proof fn producer_status_enrollment_frame_v1(before: JournalContentsV1, after: JournalContentsV1,
    request: ProducerReadV1)
    requires occupied_allocation_frame_v1(before, after), producer_status_v1(before, request).is_some(),
    ensures producer_status_v1(before, request) == producer_status_v1(after, request),
{
    let slot = request.read.allocation.slot;
    assert(before.allocations@[slot as int].is_some());
    assert(after.allocations@[slot as int] == before.allocations@[slot as int]);
}

pub proof fn enrollment_issuance_preserves_v1(before: JournalContentsV1, after: JournalContentsV1,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issuance_invariant_v1(issuance_contents_projection_v1(before, storage, history)),
        enrollment_untouched_journal_v1(before, after),
        after.allocations@.len() == before.allocations@.len(),
        after.allocation_free@.len() <= before.allocation_free@.len(),
    ensures issuance_invariant_v1(issuance_contents_projection_v1(after, storage, history)),
{
}

#[verifier::spinoff_prover]
pub proof fn enrollment_distinct_entries_v1(before: ReadContentsV1, after: ReadContentsV1,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>, remaining: usize, a: int, b: int)
    requires pending_custody_v1(before.journal), enrollment_fresh_v1(before.journal, entries),
        enrollment_final_relation_v1(before, after, entries, output, remaining),
        0 <= a < b < after.journal.allocations@.len(),
        after.journal.allocations@[a].is_some(), after.journal.allocations@[b].is_some(),
    ensures after.journal.allocations@[a].unwrap().key != after.journal.allocations@[b].unwrap().key,
{
    enrollment_final_entries_v1(before, after, entries, output, remaining);
    if before.journal.allocations@[a].is_some() {
        if before.journal.allocations@[b].is_none() {
            let j = choose|j: int| 0 <= j < entries.len() && output[j].unwrap().slot == b
                && after.journal.allocations@[b] == Some(enrollment_value_v1(entries[j]));
            assert(before.journal.allocations@[a].unwrap().key != entries[j].key);
        }
    } else {
        let i = choose|i: int| 0 <= i < entries.len() && output[i].unwrap().slot == a
            && after.journal.allocations@[a] == Some(enrollment_value_v1(entries[i]));
        if before.journal.allocations@[b].is_some() {
            assert(before.journal.allocations@[b].unwrap().key != entries[i].key);
        } else {
            let j = choose|j: int| 0 <= j < entries.len() && output[j].unwrap().slot == b
                && after.journal.allocations@[b] == Some(enrollment_value_v1(entries[j]));
            assert(i != j);
            if i < j { assert(entries[i].key != entries[j].key); }
            else { assert(entries[j].key != entries[i].key); }
        }
    }
}

#[verifier::spinoff_prover]
pub proof fn enrollment_pending_custody_preserves_v1(before: ReadContentsV1, after: ReadContentsV1,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>, remaining: usize)
    requires pending_custody_v1(before.journal), enrollment_fresh_v1(before.journal, entries),
        enrollment_final_relation_v1(before, after, entries, output, remaining),
    ensures pending_custody_v1(after.journal), occupied_allocation_frame_v1(before.journal, after.journal),
{
    enrollment_final_entries_v1(before, after, entries, output, remaining);
    enrollment_suffix_partition_v1(before, after, entries, output, remaining);
    let pre = before.journal;
    let post = after.journal;
    assert forall|a: int| 0 <= a < post.allocations@.len() implies #[trigger] allocation_custody_v1(post, a) by {
        reveal(allocation_custody_v1);
        if pre.allocations@[a].is_some() {
            assert(allocation_custody_v1(pre, a));
        } else if post.allocations@[a].is_some() {
            let i = choose|i: int| 0 <= i < entries.len() && output[i].unwrap().slot == a
                && post.allocations@[a] == Some(enrollment_value_v1(entries[i]));
            assert(enrollment_entry_error_v1(pre.context_generation, entries[i]).is_none());
        }
    }
    assert forall|m: int| 0 <= m < post.members@.len() implies #[trigger] member_custody_v1(post, m) by {
        assert(member_custody_v1(pre, m));
        reveal(member_custody_v1);
        if pre.members@[m].is_some() {
            let a = pre.members@[m].unwrap().allocation.slot;
            assert(pre.allocations@[a as int].is_some());
            assert(post.allocations@[a as int] == pre.allocations@[a as int]);
        }
    }
    assert forall|w: int| 0 <= w < post.writers@.len() implies #[trigger] writer_custody_v1(post, w) by {
        assert(writer_custody_v1(pre, w));
        reveal(writer_custody_v1);
        if pre.writers@[w].is_some() {
            let entry = pre.writers@[w].unwrap();
            if !matches!(entry, WriterEntryV1::Reserved(_)) {
                let writer = WriterReferenceV1 { slot: w as usize, key: writer_key_v1(entry) };
                let chain = choose|chain: Seq<usize>| #[trigger] retained_chain_v1(pre, writer, chain);
                retained_chain_enrollment_frame_v1(pre, post, writer, chain);
            }
        }
    }
    assert forall|a: int, b: int| 0 <= a < b < post.allocations@.len()
        && (#[trigger] post.allocations@[a]).is_some() && (#[trigger] post.allocations@[b]).is_some()
        implies post.allocations@[a].unwrap().key != post.allocations@[b].unwrap().key by {
        enrollment_distinct_entries_v1(before, after, entries, output, remaining, a, b);
    }
}

pub proof fn enrollment_preserves_issued_producer_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>, remaining: usize,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history),
        enrollment_fresh_v1(before.stable.journal, entries), producer_storage_frame_v1(before, after),
        enrollment_final_relation_v1(before.stable, after.stable, entries, output, remaining),
    ensures issued_producer_v1(after, storage, history),
        occupied_allocation_frame_v1(before.stable.journal, after.stable.journal),
{
    enrollment_pending_custody_preserves_v1(before.stable, after.stable, entries, output, remaining);
    enrollment_issuance_preserves_v1(before.stable.journal, after.stable.journal, storage, history);
    enrollment_prefix_reader_frame_v1(before.stable, entries, output, remaining, entries.len());
    reader_allocation_frame_preserves_v1(before.stable, after.stable);
    assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some()
        implies producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
        let entry = before.reservations@[s].unwrap();
        assert(producer_entry_valid_v1(before.stable.journal, entry, s, before.next_incarnation));
        reveal(producer_entry_valid_v1);
        producer_status_enrollment_frame_v1(before.stable.journal, after.stable.journal, entry.request);
    }
}

pub fn enrollment_producer_commit_exec_v1(contents: &mut ProducerReadContentsV1, entries: &[EnrollmentV1],
    output: &[Option<AllocationReferenceV1>], remaining: usize,
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    requires issued_producer_v1(*old(contents), storage, history),
        enrollment_fresh_v1(old(contents).stable.journal, entries@),
        enrollment_output_bound_v1(old(contents).stable, entries@, output@, remaining),
    ensures issued_producer_v1(*final(contents), storage, history),
        producer_storage_frame_v1(*old(contents), *final(contents)),
        enrollment_final_relation_v1(old(contents).stable, final(contents).stable, entries@, output@, remaining),
{
    let ghost before = *contents;
    enrollment_commit_suffix_exec_v1(&mut contents.stable, entries, output, remaining);
    proof {
        enrollment_preserves_issued_producer_v1(before, *contents, entries@, output@, remaining, storage, history);
    }
}

pub open spec fn enrollment_all_some_v1(values: Seq<Option<AllocationReferenceV1>>) -> bool {
    forall|i: int| 0 <= i < values.len() ==> (#[trigger] values[i]).is_some()
}

pub open spec fn enrollment_swap_relation_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>, left: usize, right: usize) -> bool {
    &&& after == before.update(left as int, before[right as int]).update(right as int, before[left as int])
    &&& after.to_multiset() == before.to_multiset()
    &&& enrollment_all_some_v1(before) ==> enrollment_all_some_v1(after)
}

pub fn enrollment_swap_exec_v1(values: &mut [Option<AllocationReferenceV1>], left: usize, right: usize)
    requires left < old(values).len(), right < old(values).len(),
    ensures enrollment_swap_relation_v1(old(values)@, final(values)@, left, right),
{
    let ghost before = values@;
    let a = values[left];
    let b = values[right];
    values[left] = b;
    values[right] = a;
    proof {
        broadcast use vstd::seq_lib::group_to_multiset_ensures;
        broadcast use vstd::multiset::group_multiset_axioms;
        vstd::seq_lib::to_multiset_update(before, left as int, b);
        vstd::seq_lib::to_multiset_update(before.update(left as int, b), right as int, a);
        if left == right { assert(values@ =~= before); }
        else { assert(values@.to_multiset() =~= before.to_multiset()); }
    }
}

pub open spec fn enrollment_heap_edge_v1(values: Seq<Option<AllocationReferenceV1>>, child: int) -> bool {
    values[(child - 1) / 2].unwrap().slot >= values[child].unwrap().slot
}

pub open spec fn enrollment_heap_from_v1(values: Seq<Option<AllocationReferenceV1>>, start: int, end: int) -> bool {
    forall|child: int| 0 < child < end && start <= (child - 1) / 2
        ==> #[trigger] enrollment_heap_edge_v1(values, child)
}

pub open spec fn enrollment_heap_hole_v1(values: Seq<Option<AllocationReferenceV1>>, start: int, end: int, hole: int) -> bool {
    &&& forall|child: int| 0 < child < end && start <= (child - 1) / 2 && (child - 1) / 2 != hole
        ==> #[trigger] enrollment_heap_edge_v1(values, child)
    &&& hole > start ==> forall|child: int| 0 < child < end && (child - 1) / 2 == hole
        ==> values[(hole - 1) / 2].unwrap().slot >= (#[trigger] values[child]).unwrap().slot
}

pub open spec fn enrollment_sift_relation_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>, start: usize, end: usize) -> bool {
    &&& after.len() == before.len()
    &&& after.to_multiset() == before.to_multiset()
    &&& enrollment_all_some_v1(after)
    &&& enrollment_heap_from_v1(after, start as int, end as int)
    &&& forall|i: int| 0 <= i < before.len() && (i < start || i >= end) ==> after[i] == before[i]
    &&& forall|i: int| start <= i < end
        ==> #[trigger] enrollment_interval_origin_v1(before, after[i], start as int, end as int)
}

pub open spec fn enrollment_interval_origin_v1(before: Seq<Option<AllocationReferenceV1>>,
    value: Option<AllocationReferenceV1>, start: int, end: int) -> bool {
    exists|j: int| start <= j < end && (#[trigger] before[j]) == value
}

#[verifier::spinoff_prover]
pub fn enrollment_sift_exec_v1(values: &mut [Option<AllocationReferenceV1>], start: usize, end: usize)
    requires start < end <= old(values).len(), enrollment_all_some_v1(old(values)@),
        enrollment_heap_from_v1(old(values)@, start + 1, end as int),
    ensures enrollment_sift_relation_v1(old(values)@, final(values)@, start, end),
{
    let ghost before = values@;
    let mut root = start;
    proof {
        assert forall|i: int| start <= i < end implies
            #[trigger] enrollment_interval_origin_v1(before, values@[i], start as int, end as int) by {
            assert(values@[i] == before[i]);
        }
    }
    while root < end / 2
        invariant start <= root < end <= values.len(), values@.len() == before.len(),
            before == old(values)@,
            root == start || start <= (root - 1) / 2,
            enrollment_all_some_v1(values@), values@.to_multiset() == before.to_multiset(),
            enrollment_heap_hole_v1(values@, start as int, end as int, root as int),
            forall|i: int| 0 <= i < before.len() && (i < start || i >= end) ==> values@[i] == before[i],
            forall|i: int| start <= i < end
                ==> #[trigger] enrollment_interval_origin_v1(before, values@[i], start as int, end as int),
        decreases end - root,
    {
        let left = root * 2 + 1;
        let mut child = left;
        if left + 1 < end && values[left].unwrap().slot < values[left + 1].unwrap().slot {
            child = left + 1;
        }
        if values[root].unwrap().slot >= values[child].unwrap().slot {
            proof {
                assert forall|c: int| 0 < c < end && (c - 1) / 2 == root
                    implies #[trigger] enrollment_heap_edge_v1(values@, c) by {
                    assert(c == left || c == left + 1);
                }
                assert(enrollment_heap_from_v1(values@, start as int, end as int));
                assert(enrollment_sift_relation_v1(before, values@, start, end));
            }
            return;
        }
        let ghost old_values = values@;
        enrollment_swap_exec_v1(values, root, child);
        proof {
            assert forall|c: int| 0 < c < end && start <= (c - 1) / 2 && (c - 1) / 2 != child
                implies #[trigger] enrollment_heap_edge_v1(values@, c) by {
                let p = (c - 1) / 2;
                if p == root {
                    assert(c == left || c == left + 1);
                } else if c == root {
                    assert(old_values[p].unwrap().slot >= old_values[child as int].unwrap().slot);
                } else {
                    assert(enrollment_heap_edge_v1(old_values, c));
                }
            }
            assert forall|c: int| 0 < c < end && (c - 1) / 2 == child implies
                values@[(child - 1) / 2].unwrap().slot >= (#[trigger] values@[c]).unwrap().slot by {
                assert(enrollment_heap_edge_v1(old_values, c));
            }
        }
        root = child;
    }
    proof {
        assert forall|c: int| 0 < c < end && start <= (c - 1) / 2
            implies #[trigger] enrollment_heap_edge_v1(values@, c) by {
            assert((c - 1) / 2 != root);
        }
        assert(enrollment_sift_relation_v1(before, values@, start, end));
    }
}

pub proof fn enrollment_heap_maximum_v1(values: Seq<Option<AllocationReferenceV1>>, end: int, node: int)
    requires 0 <= node < end <= values.len(), enrollment_heap_from_v1(values, 0, end),
    ensures values[node].unwrap().slot <= values[0].unwrap().slot,
    decreases node,
{
    if node > 0 {
        let parent = (node - 1) / 2;
        enrollment_heap_maximum_v1(values, end, parent);
        assert(enrollment_heap_edge_v1(values, node));
    }
}

pub open spec fn enrollment_sort_frontier_v1(values: Seq<Option<AllocationReferenceV1>>, end: int) -> bool {
    forall|i: int, j: int| 0 <= i < j < values.len() && end <= j
        ==> (#[trigger] values[i]).unwrap().slot <= (#[trigger] values[j]).unwrap().slot
}

pub open spec fn enrollment_sort_relation_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>) -> bool {
    &&& after.len() == before.len()
    &&& after.to_multiset() == before.to_multiset()
    &&& enrollment_all_some_v1(after)
    &&& enrollment_sort_frontier_v1(after, 0)
}

#[verifier::spinoff_prover]
pub fn enrollment_sort_slots_exec_v1(values: &mut [Option<AllocationReferenceV1>])
    requires enrollment_all_some_v1(old(values)@),
    ensures enrollment_sort_relation_v1(old(values)@, final(values)@),
{
    let ghost before = values@;
    let len = values.len();
    let mut start = len / 2;
    proof {
        assert forall|c: int| 0 < c < len && start <= (c - 1) / 2
            implies #[trigger] enrollment_heap_edge_v1(values@, c) by {
            assert((c - 1) / 2 < start);
        }
    }
    while start > 0
        invariant start <= len / 2, values.len() == len, before == old(values)@,
            values@.len() == before.len(), enrollment_all_some_v1(values@),
            values@.to_multiset() == before.to_multiset(),
            enrollment_heap_from_v1(values@, start as int, len as int),
        decreases start,
    {
        start -= 1;
        enrollment_sift_exec_v1(values, start, len);
    }
    let mut end = len;
    while end > 1
        invariant end <= len, values.len() == len, before == old(values)@,
            values@.len() == before.len(), enrollment_all_some_v1(values@),
            values@.to_multiset() == before.to_multiset(),
            enrollment_heap_from_v1(values@, 0, end as int),
            enrollment_sort_frontier_v1(values@, end as int),
        decreases end,
    {
        end -= 1;
        let ghost previous = values@;
        proof {
            assert forall|i: int| 0 <= i <= end implies
                (#[trigger] previous[i]).unwrap().slot <= previous[0].unwrap().slot by {
                enrollment_heap_maximum_v1(previous, end + 1, i);
            }
        }
        enrollment_swap_exec_v1(values, 0, end);
        let ghost swapped = values@;
        proof {
            assert forall|c: int| 0 < c < end && 1 <= (c - 1) / 2 implies
                #[trigger] enrollment_heap_edge_v1(swapped, c) by {
                assert(enrollment_heap_edge_v1(previous, c));
            }
            assert forall|i: int| 0 <= i < end implies
                (#[trigger] swapped[i]).unwrap().slot <= previous[0].unwrap().slot by {}
        }
        enrollment_sift_exec_v1(values, 0, end);
        proof {
            assert forall|i: int| 0 <= i <= end implies
                (#[trigger] values@[i]).unwrap().slot <= previous[0].unwrap().slot by {
                if i < end {
                    assert(enrollment_interval_origin_v1(swapped, values@[i], 0, end as int));
                    let p = choose|p: int| 0 <= p < end && swapped[p] == values@[i];
                    assert(swapped[p].unwrap().slot <= previous[0].unwrap().slot);
                }
            }
            assert forall|i: int, j: int| 0 <= i < j < values@.len() && end <= j implies
                (#[trigger] values@[i]).unwrap().slot <= (#[trigger] values@[j]).unwrap().slot by {
                if j == end {
                    assert(values@[j] == previous[0]);
                } else if i <= end {
                    assert(previous[0].unwrap().slot <= previous[j].unwrap().slot);
                } else {
                    assert(previous[i].unwrap().slot <= previous[j].unwrap().slot);
                }
            }
        }
    }
}

pub open spec fn enrollment_keys_sorted_v1(entries: Seq<EnrollmentV1>) -> bool {
    forall|i: int, j: int| 0 <= i < j < entries.len()
        ==> !enrollment_key_less_v1((#[trigger] entries[j]).key, (#[trigger] entries[i]).key)
}

#[verifier::spinoff_prover]
pub fn enrollment_contains_key_exec_v1(entries: &[EnrollmentV1], key: AllocationKeyV1) -> (found: bool)
    requires enrollment_keys_sorted_v1(entries@),
    ensures found == (exists|i: int| 0 <= i < entries@.len() && (#[trigger] entries@[i]).key == key),
{
    let mut lo = 0usize;
    let mut hi = entries.len();
    while lo < hi
        invariant lo <= hi <= entries.len(), enrollment_keys_sorted_v1(entries@),
            forall|i: int| 0 <= i < lo ==> enrollment_key_less_v1((#[trigger] entries@[i]).key, key),
            forall|i: int| hi <= i < entries@.len() ==> !enrollment_key_less_v1((#[trigger] entries@[i]).key, key),
        decreases hi - lo,
    {
        let mid = lo + (hi - lo) / 2;
        if enrollment_key_less_exec_v1(entries[mid].key, key) {
            proof {
                assert forall|i: int| 0 <= i <= mid implies enrollment_key_less_v1((#[trigger] entries@[i]).key, key) by {
                    if lo <= i < mid {
                        assert(!enrollment_key_less_v1(entries@[mid as int].key, entries@[i].key));
                    }
                }
            }
            lo = mid + 1;
        } else {
            proof {
                assert forall|i: int| mid <= i < entries@.len() implies !enrollment_key_less_v1((#[trigger] entries@[i]).key, key) by {
                    if mid < i < hi {
                        assert(!enrollment_key_less_v1(entries@[i].key, entries@[mid as int].key));
                    }
                }
            }
            hi = mid;
        }
    }
    if lo == entries.len() { return false; }
    let entry = entries[lo];
    let found = entry.key.context_generation == key.context_generation && entry.key.local == key.local;
    proof {
        if !found {
            assert forall|i: int| 0 <= i < entries@.len() implies (#[trigger] entries@[i]).key != key by {
                if lo < i { assert(!enrollment_key_less_v1(entries@[i].key, entries@[lo as int].key)); }
            }
        } else {
            assert(entries@[lo as int].key == key);
        }
    }
    found
}

#[verifier::spinoff_prover]
pub fn enrollment_contains_slot_exec_v1(values: &[Option<AllocationReferenceV1>], slot: usize) -> (found: bool)
    requires enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
    ensures found == (exists|i: int| 0 <= i < values@.len() && (#[trigger] values@[i]).unwrap().slot == slot),
{
    let mut lo = 0usize;
    let mut hi = values.len();
    while lo < hi
        invariant lo <= hi <= values.len(), enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
            forall|i: int| 0 <= i < lo ==> (#[trigger] values@[i]).unwrap().slot < slot,
            forall|i: int| hi <= i < values@.len() ==> (#[trigger] values@[i]).unwrap().slot >= slot,
        decreases hi - lo,
    {
        let mid = lo + (hi - lo) / 2;
        if values[mid].unwrap().slot < slot {
            proof {
                assert forall|i: int| 0 <= i <= mid implies (#[trigger] values@[i]).unwrap().slot < slot by {
                    if lo <= i < mid { assert(values@[i].unwrap().slot <= values@[mid as int].unwrap().slot); }
                }
            }
            lo = mid + 1;
        } else {
            proof {
                assert forall|i: int| mid <= i < values@.len() implies (#[trigger] values@[i]).unwrap().slot >= slot by {
                    if mid < i < hi { assert(values@[mid as int].unwrap().slot <= values@[i].unwrap().slot); }
                }
            }
            hi = mid;
        }
    }
    if lo == values.len() { return false; }
    let found = values[lo].unwrap().slot == slot;
    proof {
        if !found {
            assert forall|i: int| 0 <= i < values@.len() implies (#[trigger] values@[i]).unwrap().slot != slot by {
                if lo < i { assert(values@[lo as int].unwrap().slot <= values@[i].unwrap().slot); }
            }
        } else { assert(values@[lo as int].unwrap().slot == slot); }
    }
    found
}

pub open spec fn enrollment_entry_admitted_v1(context: u64, entries: Seq<EnrollmentV1>, i: int) -> bool {
    enrollment_entry_error_v1(context, entries[i]).is_none()
        && (i == 0 || enrollment_key_less_v1(entries[i - 1].key, entries[i].key))
}

pub proof fn enrollment_scan_accepts_v1(context: u64, entries: Seq<EnrollmentV1>, index: nat)
    requires index <= entries.len(), enrollment_roster_scan_v1(context, entries, index) == Ok(()),
    ensures forall|i: int| index <= i < entries.len() ==> #[trigger] enrollment_entry_admitted_v1(context, entries, i),
    decreases entries.len() - index,
{
    if index < entries.len() {
        enrollment_scan_accepts_v1(context, entries, index + 1);
    }
}

pub open spec fn enrollment_keys_strict_v1(entries: Seq<EnrollmentV1>) -> bool {
    forall|i: int, j: int| 0 <= i < j < entries.len()
        ==> enrollment_key_less_v1((#[trigger] entries[i]).key, (#[trigger] entries[j]).key)
}

pub proof fn enrollment_roster_order_v1(context: u64, entries: Seq<EnrollmentV1>, i: int, j: int)
    requires 0 <= i < j < entries.len(),
        forall|a: int| 0 <= a < entries.len() ==> #[trigger] enrollment_entry_admitted_v1(context, entries, a),
    ensures enrollment_key_less_v1(entries[i].key, entries[j].key),
    decreases j - i,
{
    assert(enrollment_entry_admitted_v1(context, entries, j));
    if i + 1 < j { enrollment_roster_order_v1(context, entries, i, j - 1); }
}

pub open spec fn enrollment_header_ready_v1(context: u64, capacity: usize, free_len: usize,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>) -> bool {
    &&& entries.len() <= capacity
    &&& output.len() == entries.len()
    &&& forall|i: int| 0 <= i < output.len() ==> (#[trigger] output[i]).is_none()
    &&& forall|i: int| 0 <= i < entries.len() ==> enrollment_entry_error_v1(context, #[trigger] entries[i]).is_none()
    &&& enrollment_keys_strict_v1(entries)
    &&& enrollment_keys_sorted_v1(entries)
    &&& entries.len() > 0 ==> free_len <= capacity
}

pub proof fn enrollment_header_success_v1(context: u64, capacity: usize, free_len: usize,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>)
    requires enrollment_header_decision_v1(context, capacity, free_len, entries, output) == Ok(()),
    ensures enrollment_header_ready_v1(context, capacity, free_len, entries, output),
{
    let scan = enrollment_roster_scan_v1(context, entries, 0);
    assert(scan == Ok(())) by {
        match scan {
            Err(error) => { assert(enrollment_header_decision_v1(context, capacity, free_len, entries, output) == Err(error)); }
            Ok(value) => { assert(value == ()); },
        }
    }
    enrollment_scan_accepts_v1(context, entries, 0);
    assert forall|i: int| 0 <= i < entries.len() implies
        enrollment_entry_error_v1(context, #[trigger] entries[i]).is_none() by {
        assert(enrollment_entry_admitted_v1(context, entries, i));
    }
    assert forall|i: int, j: int| 0 <= i < j < entries.len() implies
        enrollment_key_less_v1((#[trigger] entries[i]).key, (#[trigger] entries[j]).key) by {
        enrollment_roster_order_v1(context, entries, i, j);
    }
    assert forall|i: int, j: int| 0 <= i < j < entries.len() implies
        !enrollment_key_less_v1((#[trigger] entries[j]).key, (#[trigger] entries[i]).key) by {
        assert(enrollment_key_less_v1(entries[i].key, entries[j].key));
    }
}

pub open spec fn enrollment_plan_output_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>)
    -> Seq<Option<AllocationReferenceV1>>
    recommends entries.len() <= journal.allocation_free@.len(),
{
    Seq::new(entries.len(), |i: int| Some(AllocationReferenceV1 {
        slot: journal.allocation_free@[journal.allocation_free@.len() - 1 - i], key: entries[i].key,
    }))
}

pub fn enrollment_fill_plan_exec_v1(journal: &JournalContentsV1, entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>])
    requires entries.len() <= journal.allocation_free@.len(), old(output).len() == entries.len(),
    ensures final(output)@ == enrollment_plan_output_v1(*journal, entries@),
{
    let mut index = 0usize;
    while index < entries.len()
        invariant index <= entries.len() <= journal.allocation_free@.len(), output.len() == entries.len(),
            forall|i: int| 0 <= i < index ==> (#[trigger] output@[i]) == enrollment_plan_output_v1(*journal, entries@)[i],
        decreases entries.len() - index,
    {
        let slot = journal.allocation_free[journal.allocation_free.len() - 1 - index];
        output[index] = Some(AllocationReferenceV1 { slot, key: entries[index].key });
        index += 1;
    }
    proof { assert(output@ =~= enrollment_plan_output_v1(*journal, entries@)); }
}

pub fn enrollment_clear_output_exec_v1(output: &mut [Option<AllocationReferenceV1>])
    ensures final(output)@ == Seq::new(old(output)@.len(), |i: int| None::<AllocationReferenceV1>),
{
    let mut index = 0usize;
    while index < output.len()
        invariant index <= output.len(), output@.len() == old(output)@.len(),
            forall|i: int| 0 <= i < index ==> (#[trigger] output@[i]).is_none(),
        decreases output.len() - index,
    {
        output[index] = None;
        index += 1;
    }
    proof { assert(output@ =~= Seq::new(old(output)@.len(), |i: int| None::<AllocationReferenceV1>)); }
}

pub proof fn enrollment_permutation_contains_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>, value: Option<AllocationReferenceV1>)
    requires before.to_multiset() == after.to_multiset(),
    ensures before.contains(value) == after.contains(value),
{
    vstd::seq_lib::to_multiset_contains(before, value);
    vstd::seq_lib::to_multiset_contains(after, value);
}

pub proof fn enrollment_permutation_unique_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>)
    requires before.to_multiset() == after.to_multiset(), before.no_duplicates(),
    ensures after.no_duplicates(),
{
    before.lemma_multiset_has_no_duplicates();
    after.lemma_multiset_has_no_duplicates_conv();
}

pub open spec fn enrollment_slots_distinct_v1(values: Seq<Option<AllocationReferenceV1>>) -> bool {
    forall|i: int, j: int| 0 <= i < j < values.len()
        ==> (#[trigger] values[i]).unwrap().slot != (#[trigger] values[j]).unwrap().slot
}

#[verifier::spinoff_prover]
pub proof fn enrollment_permutation_distinct_slots_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>)
    requires enrollment_all_some_v1(before), enrollment_all_some_v1(after),
        before.to_multiset() == after.to_multiset(), before.no_duplicates(),
    ensures enrollment_slots_distinct_v1(before) == enrollment_slots_distinct_v1(after),
{
    enrollment_permutation_unique_v1(before, after);
    if enrollment_slots_distinct_v1(before) {
        assert forall|i: int, j: int| 0 <= i < j < after.len() implies
            (#[trigger] after[i]).unwrap().slot != (#[trigger] after[j]).unwrap().slot by {
            assert(after.contains(after[i]));
            assert(after.contains(after[j]));
            enrollment_permutation_contains_v1(before, after, after[i]);
            enrollment_permutation_contains_v1(before, after, after[j]);
            let a = choose|a: int| 0 <= a < before.len() && before[a] == after[i];
            let b = choose|b: int| 0 <= b < before.len() && before[b] == after[j];
            assert(a != b);
            if a < b { assert(before[a].unwrap().slot != before[b].unwrap().slot); }
            else { assert(before[b].unwrap().slot != before[a].unwrap().slot); }
        }
    }
    if enrollment_slots_distinct_v1(after) {
        assert forall|i: int, j: int| 0 <= i < j < before.len() implies
            (#[trigger] before[i]).unwrap().slot != (#[trigger] before[j]).unwrap().slot by {
            assert(before.contains(before[i]));
            assert(before.contains(before[j]));
            enrollment_permutation_contains_v1(before, after, before[i]);
            enrollment_permutation_contains_v1(before, after, before[j]);
            let a = choose|a: int| 0 <= a < after.len() && after[a] == before[i];
            let b = choose|b: int| 0 <= b < after.len() && after[b] == before[j];
            assert(a != b);
            if a < b { assert(after[a].unwrap().slot != after[b].unwrap().slot); }
            else { assert(after[b].unwrap().slot != after[a].unwrap().slot); }
        }
    }
}

pub proof fn enrollment_permutation_slot_set_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>, slot: usize)
    requires enrollment_all_some_v1(before), enrollment_all_some_v1(after), before.to_multiset() == after.to_multiset(),
    ensures (exists|i: int| 0 <= i < before.len() && (#[trigger] before[i]).unwrap().slot == slot)
        == (exists|i: int| 0 <= i < after.len() && (#[trigger] after[i]).unwrap().slot == slot),
{
    if exists|i: int| 0 <= i < before.len() && before[i].unwrap().slot == slot {
        let i = choose|i: int| 0 <= i < before.len() && before[i].unwrap().slot == slot;
        assert(before.contains(before[i]));
        enrollment_permutation_contains_v1(before, after, before[i]);
        let j = choose|j: int| 0 <= j < after.len() && after[j] == before[i];
        assert(after[j].unwrap().slot == slot);
    }
    if exists|i: int| 0 <= i < after.len() && after[i].unwrap().slot == slot {
        let i = choose|i: int| 0 <= i < after.len() && after[i].unwrap().slot == slot;
        assert(after.contains(after[i]));
        enrollment_permutation_contains_v1(before, after, after[i]);
        let j = choose|j: int| 0 <= j < before.len() && before[j] == after[i];
        assert(before[j].unwrap().slot == slot);
    }
}

pub fn enrollment_duplicate_slots_exec_v1(values: &[Option<AllocationReferenceV1>]) -> (duplicate: bool)
    requires enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
    ensures duplicate == !enrollment_slots_distinct_v1(values@),
{
    if values.len() == 0 { return false; }
    let mut index = 1usize;
    while index < values.len()
        invariant 1 <= index <= values.len(), enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
            forall|i: int| 0 < i < index ==> (#[trigger] values@[i - 1]).unwrap().slot != (#[trigger] values@[i]).unwrap().slot,
        decreases values.len() - index,
    {
        if values[index - 1].unwrap().slot == values[index].unwrap().slot { return true; }
        index += 1;
    }
    proof {
        assert forall|i: int, j: int| 0 <= i < j < values@.len() implies
            (#[trigger] values@[i]).unwrap().slot != (#[trigger] values@[j]).unwrap().slot by {
            if i < j - 1 { assert(values@[i].unwrap().slot <= values@[j - 1].unwrap().slot); }
            assert(values@[j - 1].unwrap().slot <= values@[j].unwrap().slot);
        }
    }
    false
}

pub open spec fn enrollment_replay_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>) -> bool {
    exists|a: int, i: int| 0 <= a < journal.allocations@.len() && 0 <= i < entries.len()
        && (#[trigger] journal.allocations@[a]).is_some()
        && journal.allocations@[a].unwrap().key == (#[trigger] entries[i]).key
}

pub fn enrollment_replay_exec_v1(journal: &JournalContentsV1, entries: &[EnrollmentV1]) -> (replay: bool)
    requires enrollment_keys_sorted_v1(entries@),
    ensures replay == enrollment_replay_v1(*journal, entries@),
{
    let mut index = 0usize;
    while index < journal.allocations.len()
        invariant index <= journal.allocations@.len(), enrollment_keys_sorted_v1(entries@),
            forall|a: int, i: int| 0 <= a < index && 0 <= i < entries@.len()
                && (#[trigger] journal.allocations@[a]).is_some()
                ==> journal.allocations@[a].unwrap().key != (#[trigger] entries@[i]).key,
        decreases journal.allocations.len() - index,
    {
        if let Some(entry) = journal.allocations[index] {
            if enrollment_contains_key_exec_v1(entries, entry.key) { return true; }
        }
        index += 1;
    }
    false
}

pub open spec fn enrollment_selected_vacant_v1(journal: JournalContentsV1, count: nat) -> bool {
    forall|i: int| journal.allocation_free@.len() - count <= i < journal.allocation_free@.len() ==> {
        &&& (#[trigger] journal.allocation_free@[i]) < journal.allocations@.len()
        &&& journal.allocations@[journal.allocation_free@[i] as int].is_none()
    }
}

pub fn enrollment_selected_vacant_exec_v1(journal: &JournalContentsV1, remaining: usize) -> (vacant: bool)
    requires remaining <= journal.allocation_free@.len(),
    ensures vacant == enrollment_selected_vacant_v1(*journal, (journal.allocation_free@.len() - remaining) as nat),
{
    let mut index = remaining;
    while index < journal.allocation_free.len()
        invariant remaining <= index <= journal.allocation_free@.len(),
            forall|i: int| remaining <= i < index ==> {
                &&& (#[trigger] journal.allocation_free@[i]) < journal.allocations@.len()
                &&& journal.allocations@[journal.allocation_free@[i] as int].is_none()
            },
        decreases journal.allocation_free.len() - index,
    {
        let slot = journal.allocation_free[index];
        if slot >= journal.allocations.len() || journal.allocations[slot].is_some() { return false; }
        index += 1;
    }
    true
}

pub open spec fn enrollment_retained_clear_v1(journal: JournalContentsV1,
    values: Seq<Option<AllocationReferenceV1>>, remaining: usize) -> bool {
    forall|a: int, i: int| 0 <= a < remaining && 0 <= i < values.len()
        ==> (#[trigger] journal.allocation_free@[a]) != (#[trigger] values[i]).unwrap().slot
}

pub fn enrollment_retained_clear_exec_v1(journal: &JournalContentsV1,
    values: &[Option<AllocationReferenceV1>], remaining: usize) -> (clear: bool)
    requires remaining <= journal.allocation_free@.len(), enrollment_all_some_v1(values@),
        enrollment_sort_frontier_v1(values@, 0),
    ensures clear == enrollment_retained_clear_v1(*journal, values@, remaining),
{
    let mut index = 0usize;
    while index < remaining
        invariant index <= remaining <= journal.allocation_free@.len(),
            enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
            forall|a: int, i: int| 0 <= a < index && 0 <= i < values@.len()
                ==> (#[trigger] journal.allocation_free@[a]) != (#[trigger] values@[i]).unwrap().slot,
        decreases remaining - index,
    {
        if enrollment_contains_slot_exec_v1(values, journal.allocation_free[index]) { return false; }
        index += 1;
    }
    true
}

pub proof fn enrollment_retained_permutation_v1(journal: JournalContentsV1,
    before: Seq<Option<AllocationReferenceV1>>, after: Seq<Option<AllocationReferenceV1>>, remaining: usize)
    requires remaining <= journal.allocation_free@.len(), enrollment_all_some_v1(before), enrollment_all_some_v1(after),
        before.to_multiset() == after.to_multiset(),
    ensures enrollment_retained_clear_v1(journal, before, remaining) == enrollment_retained_clear_v1(journal, after, remaining),
{
    if enrollment_retained_clear_v1(journal, before, remaining) {
        assert forall|a: int, i: int| 0 <= a < remaining && 0 <= i < after.len() implies
            (#[trigger] journal.allocation_free@[a]) != (#[trigger] after[i]).unwrap().slot by {
            enrollment_permutation_slot_set_v1(before, after, journal.allocation_free@[a]);
        }
    }
    if enrollment_retained_clear_v1(journal, after, remaining) {
        assert forall|a: int, i: int| 0 <= a < remaining && 0 <= i < before.len() implies
            (#[trigger] journal.allocation_free@[a]) != (#[trigger] before[i]).unwrap().slot by {
            enrollment_permutation_slot_set_v1(before, after, journal.allocation_free@[a]);
        }
    }
}

pub proof fn enrollment_plan_unique_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>)
    requires entries.len() <= journal.allocation_free@.len(), enrollment_keys_strict_v1(entries),
    ensures enrollment_all_some_v1(enrollment_plan_output_v1(journal, entries)),
        enrollment_plan_output_v1(journal, entries).no_duplicates(),
{
    let values = enrollment_plan_output_v1(journal, entries);
    assert forall|i: int, j: int| 0 <= i < j < values.len() implies values[i] != values[j] by {
        assert(enrollment_key_less_v1(entries[i].key, entries[j].key));
    }
}

pub open spec fn enrollment_decision_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>) -> Result<(), EnrollmentErrorV1>
{
    match enrollment_header_decision_v1(journal.context_generation, journal.allocation_capacity,
        journal.allocation_free@.len() as usize, entries, output) {
        Err(error) => Err(error),
        Ok(()) => {
            if entries.len() == 0 { Ok(()) }
            else if enrollment_replay_v1(journal, entries) { Err(EnrollmentErrorV1::AllocationReplay) }
            else if entries.len() > journal.allocation_free@.len() { Err(EnrollmentErrorV1::AllocationCapacity) }
            else if !enrollment_selected_vacant_v1(journal, entries.len()) { Err(EnrollmentErrorV1::InvalidState) }
            else {
                let plan = enrollment_plan_output_v1(journal, entries);
                let remaining = (journal.allocation_free@.len() - entries.len()) as usize;
                if !enrollment_slots_distinct_v1(plan) || !enrollment_retained_clear_v1(journal, plan, remaining) {
                    Err(EnrollmentErrorV1::InvalidState)
                } else { Ok(()) }
            }
        },
    }
}

pub open spec fn enrollment_execution_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    entries: Seq<EnrollmentV1>, old_output: Seq<Option<AllocationReferenceV1>>,
    output: Seq<Option<AllocationReferenceV1>>, result: Result<(), EnrollmentErrorV1>) -> bool {
    &&& result == enrollment_decision_v1(before, entries, old_output)
    &&& match result {
        Err(_) => after == before && output == old_output,
        Ok(()) => {
            &&& output == enrollment_plan_output_v1(before, entries)
            &&& after.allocations@ == enrollment_prefix_v1(before.allocations@, entries, output, entries.len())
            &&& after.allocation_free@ == before.allocation_free@.subrange(0, before.allocation_free@.len() - entries.len())
            &&& enrollment_untouched_journal_v1(before, after)
        },
    }
}

#[verifier::spinoff_prover]
pub fn enrollment_journal_exec_v1(contents: &mut JournalContentsV1, entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>]) -> (result: Result<(), EnrollmentErrorV1>)
    ensures enrollment_execution_relation_v1(*old(contents), *final(contents), entries@, old(output)@, final(output)@, result),
{
    let ghost before = *contents;
    let ghost original = output@;
    if let Err(error) = enrollment_header_exec_v1(contents.context_generation, contents.allocation_capacity,
        contents.allocation_free.len(), entries, output) {
        return Err(error);
    }
    proof { enrollment_header_success_v1(contents.context_generation, contents.allocation_capacity,
        contents.allocation_free.len(), entries@, output@); }
    if entries.len() == 0 {
        proof {
            assert(output@ =~= enrollment_plan_output_v1(before, entries@));
            assert(contents.allocation_free@ =~= before.allocation_free@.subrange(0, before.allocation_free@.len() as int));
        }
        return Ok(());
    }
    if enrollment_replay_exec_v1(contents, entries) { return Err(EnrollmentErrorV1::AllocationReplay); }
    if entries.len() > contents.allocation_free.len() { return Err(EnrollmentErrorV1::AllocationCapacity); }
    let remaining = contents.allocation_free.len() - entries.len();
    if !enrollment_selected_vacant_exec_v1(contents, remaining) { return Err(EnrollmentErrorV1::InvalidState); }
    enrollment_fill_plan_exec_v1(contents, entries, output);
    let ghost plan = output@;
    proof { enrollment_plan_unique_v1(before, entries@); }
    enrollment_sort_slots_exec_v1(output);
    proof {
        enrollment_permutation_distinct_slots_v1(plan, output@);
        enrollment_retained_permutation_v1(before, plan, output@, remaining);
    }
    if enrollment_duplicate_slots_exec_v1(output) || !enrollment_retained_clear_exec_v1(contents, output, remaining) {
        enrollment_clear_output_exec_v1(output);
        proof { assert(output@ =~= original); }
        return Err(EnrollmentErrorV1::InvalidState);
    }
    enrollment_fill_plan_exec_v1(contents, entries, output);
    proof { assert(enrollment_journal_output_bound_v1(before, entries@, output@, remaining)); }
    enrollment_commit_journal_exec_v1(contents, entries, output, remaining);
    Ok(())
}

pub proof fn enrollment_success_admission_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>,
    original: Seq<Option<AllocationReferenceV1>>)
    requires enrollment_decision_v1(journal, entries, original) == Ok(()),
        journal.allocation_free@.len() <= usize::MAX,
    ensures enrollment_fresh_v1(journal, entries),
        entries.len() <= journal.allocation_free@.len(),
        enrollment_journal_output_bound_v1(journal, entries, enrollment_plan_output_v1(journal, entries),
            (journal.allocation_free@.len() - entries.len()) as usize),
{
    let header = enrollment_header_decision_v1(journal.context_generation, journal.allocation_capacity,
        journal.allocation_free@.len() as usize, entries, original);
    assert(header == Ok(())) by {
        match header { Ok(value) => { assert(value == ()); }, Err(_) => {}, }
    }
    enrollment_header_success_v1(journal.context_generation, journal.allocation_capacity,
        journal.allocation_free@.len() as usize, entries, original);
    assert forall|i: int, j: int| 0 <= i < j < entries.len() implies
        (#[trigger] entries[i]).key != (#[trigger] entries[j]).key by {
        assert(enrollment_key_less_v1(entries[i].key, entries[j].key));
    }
}

pub open spec fn enrollment_issued_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>, output: Seq<Option<AllocationReferenceV1>>,
    result: Result<(), EnrollmentErrorV1>, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>) -> bool {
    &&& enrollment_execution_relation_v1(before.stable.journal, after.stable.journal, entries, original, output, result)
    &&& producer_storage_frame_v1(before, after)
    &&& issued_producer_v1(after, storage, history)
    &&& result.is_err() ==> after == before
    &&& forall|request: ProducerReadV1| producer_status_v1(before.stable.journal, request).is_some()
        ==> #[trigger] producer_status_v1(after.stable.journal, request) == producer_status_v1(before.stable.journal, request)
}

#[verifier::spinoff_prover]
pub fn enrollment_issued_exec_v1(contents: &mut ProducerReadContentsV1, entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>], Ghost(storage): Ghost<StorageCapacitiesV1>,
    Ghost(history): Ghost<Seq<WriterReferenceV1>>) -> (result: Result<(), EnrollmentErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures enrollment_issued_relation_v1(*old(contents), *final(contents), entries@, old(output)@, final(output)@,
        result, storage, history),
{
    let ghost before = *contents;
    let ghost original = output@;
    let result = enrollment_journal_exec_v1(&mut contents.stable.journal, entries, output);
    proof {
        if let Ok(value) = result {
            assert(value == ());
            enrollment_success_admission_v1(before.stable.journal, entries@, original);
            let remaining = (before.stable.journal.allocation_free@.len() - entries@.len()) as usize;
            enrollment_preserves_issued_producer_v1(before, *contents, entries@, output@, remaining, storage, history);
            assert forall|request: ProducerReadV1| producer_status_v1(before.stable.journal, request).is_some()
                implies #[trigger] producer_status_v1(contents.stable.journal, request)
                    == producer_status_v1(before.stable.journal, request) by {
                producer_status_enrollment_frame_v1(before.stable.journal, contents.stable.journal, request);
            }
        }
    }
    result
}

#[verifier::spinoff_prover]
pub fn enrollment_constructor_witness_v1() -> (result: bool)
    ensures result,
{
    proof {
        reveal_with_fuel(enrollment_roster_scan_v1, 4);
        reveal_with_fuel(enrollment_prefix_v1, 4);
    }
    let ghost storage = witness_storage_v1(3, 1);
    let ghost history = Seq::<WriterReferenceV1>::empty();
    let mut contents = match issued_producer_constructor_exec_v1(7, 3, 1, 2, Ghost(storage)) {
        Ok(contents) => contents, Err(_) => return false,
    };
    let entries = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 },
        EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
        device: DeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let mut output = vec![None, None];
    let enrolled = enrollment_issued_exec_v1(&mut contents, &entries, &mut output, Ghost(storage), Ghost(history));
    assert(enrolled == Ok(()));
    assert(output@[0] == Some(AllocationReferenceV1 { slot: 0, key: entries@[0].key }));
    assert(output@[1] == Some(AllocationReferenceV1 { slot: 1, key: entries@[1].key }));
    assert(contents.stable.journal.allocations@[0] == Some(enrollment_value_v1(entries@[0])));
    assert(contents.stable.journal.allocations@[1] == Some(enrollment_value_v1(entries@[1])));
    let mut replay_output = vec![None, None];
    let ghost snapshot = contents;
    let replay = enrollment_issued_exec_v1(&mut contents, &entries, &mut replay_output, Ghost(storage), Ghost(history));
    assert(replay == Err(EnrollmentErrorV1::AllocationReplay));
    assert(contents == snapshot && replay_output@ == seq![None, None]);
    let lower = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 10 },
        device: DeviceKeyV1 { context_generation: 7, local: 4 }, byte_extent: 64 }];
    let mut last = vec![None];
    let enrolled_lower = enrollment_issued_exec_v1(&mut contents, &lower, &mut last, Ghost(storage), Ghost(history));
    assert(enrolled_lower == Ok(()));
    assert(last@[0] == Some(AllocationReferenceV1 { slot: 2, key: lower@[0].key }));
    assert(contents.stable.journal.allocation_free@.len() == 0);
    let key = WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission };
    let writer = match register_issued_producer_exec_v1(&mut contents, key, Ghost(storage), Ghost(history)) {
        Ok(reference) => reference, Err(_) => return false,
    };
    assert(writer.slot == 0 && contents.stable.journal.reserved_count == 1);
    assert(issued_producer_v1(contents, storage, seq![writer]));
    true
}

}

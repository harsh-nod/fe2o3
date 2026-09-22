use self::ContextVersionJournalV1 as JournalContentsV1;
use self::ContextVersionJournalErrorV1 as EnrollmentErrorV1;

verus! {

spec fn issuable_id_v1(id: u64) -> bool { id > 0 && id < u64::MAX }

// Test-only instrumentation is outside the journal state and absent in non-test builds.
fn enrollment_indexed_access_v1(journal: &JournalContentsV1) {}

spec fn enrollment_entry_error_v1(context: u64, entry: EnrollmentV1) -> Option<EnrollmentErrorV1> {
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

fn enrollment_entry_error_exec_v1(context: u64, entry: EnrollmentV1) -> (result: Option<EnrollmentErrorV1>)
    ensures result == enrollment_entry_error_v1(context, entry),
{
    enrollment_entry_error_body!(context, entry)
}

spec fn enrollment_roster_scan_v1(context: u64, entries: Seq<EnrollmentV1>, index: nat)
    -> Result<(), EnrollmentErrorV1>
    decreases entries.len() - index,
{
    if index >= entries.len() { Ok(()) }
    else if let Some(error) = enrollment_entry_error_v1(context, entries[index as int]) { Err(error) }
    else if index > 0 && !enrollment_key_less_v1(entries[index - 1].key, entries[index as int].key) {
        Err(EnrollmentErrorV1::NonCanonicalRoster)
    } else { enrollment_roster_scan_v1(context, entries, index + 1) }
}

spec fn enrollment_header_decision_v1(context: u64, capacity: usize, free_len: usize,
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
fn enrollment_header_exec_v1(context: u64, capacity: usize, free_len: usize,
    entries: &[EnrollmentV1], output: &mut [Option<AllocationReferenceV1>])
    -> (result: Result<(), EnrollmentErrorV1>)
    ensures result == enrollment_header_decision_v1(context, capacity, free_len, entries@, old(output)@),
        final(output)@ == old(output)@,
{
    enrollment_header_body!(verus_exec_expr, context, capacity, free_len, entries, output, index, previous, [
        invariant index <= output.len(), output@ == old(output)@,
            entries.len() <= capacity, output.len() == entries.len(),
            forall|i: int| 0 <= i < index ==> output@[i].is_none(),
        decreases output.len() - index,
    ], [
        invariant index <= entries.len(), output@ == old(output)@,
            previous == if index == 0 { None } else { Some(entries@[index - 1].key) },
            output.len() == entries.len(), entries.len() <= capacity,
            forall|i: int| 0 <= i < output.len() ==> output@[i].is_none(),
            enrollment_roster_scan_v1(context, entries@, 0)
                == enrollment_roster_scan_v1(context, entries@, index as nat),
        decreases entries.len() - index,
    ])
}

spec fn enrollment_value_v1(entry: EnrollmentV1) -> AllocationEntryV1 {
    AllocationEntryV1 { key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
        attempt_epoch: 0, content_lineage: 0, pending_member: None }
}

spec fn enrollment_journal_output_bound_v1(before: JournalContentsV1, entries: Seq<EnrollmentV1>,
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

spec fn enrollment_prefix_v1(allocations: Seq<Option<AllocationEntryV1>>, entries: Seq<EnrollmentV1>,
    output: Seq<Option<AllocationReferenceV1>>, count: nat) -> Seq<Option<AllocationEntryV1>>
    recommends count <= entries.len(), entries.len() == output.len(),
    decreases count,
{
    if count == 0 { allocations }
    else { enrollment_prefix_v1(allocations, entries, output, (count - 1) as nat)
        .update(output[count - 1].unwrap().slot as int, Some(enrollment_value_v1(entries[count - 1]))) }
}

spec fn enrollment_untouched_journal_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.registration_watermark == before.registration_watermark
    &&& after.reserved_count == before.reserved_count
    &&& true
    &&& after.free@ == before.free@
    &&& after.members@ == before.members@
    &&& after.member_free@ == before.member_free@
    &&& after.scratch@ == before.scratch@
}

fn enrollment_commit_journal_exec_v1(contents: &mut JournalContentsV1, entries: &[EnrollmentV1],
    output: &[Option<AllocationReferenceV1>], remaining: usize)
    requires enrollment_journal_output_bound_v1(*old(contents), entries@, output@, remaining),
    ensures enrollment_untouched_journal_v1(*old(contents), *final(contents)),
        final(contents).allocations@ == enrollment_prefix_v1(old(contents).allocations@, entries@, output@, entries@.len()),
        final(contents).allocation_free@ == old(contents).allocation_free@.subrange(0, remaining as int),
{
    let ghost before = *contents;
    enrollment_commit_journal_body!(verus_exec_expr, contents, entries, output, remaining, index, [
        invariant index <= entries.len(), before == *old(contents),
            enrollment_journal_output_bound_v1(before, entries@, output@, remaining),
            enrollment_untouched_journal_v1(before, *contents),
            contents.allocation_free@ == before.allocation_free@,
            contents.allocations@ == enrollment_prefix_v1(before.allocations@, entries@, output@, index as nat),
            contents.allocations@.len() == before.allocations@.len(),
        decreases entries.len() - index,
    ])
}

spec fn enrollment_entry_admitted_v1(context: u64, entries: Seq<EnrollmentV1>, i: int) -> bool {
    enrollment_entry_error_v1(context, entries[i]).is_none()
        && (i == 0 || enrollment_key_less_v1(entries[i - 1].key, entries[i].key))
}

proof fn enrollment_scan_accepts_v1(context: u64, entries: Seq<EnrollmentV1>, index: nat)
    requires index <= entries.len(), enrollment_roster_scan_v1(context, entries, index) == Ok(()),
    ensures forall|i: int| index <= i < entries.len() ==> #[trigger] enrollment_entry_admitted_v1(context, entries, i),
    decreases entries.len() - index,
{
    if index < entries.len() {
        enrollment_scan_accepts_v1(context, entries, index + 1);
    }
}

spec fn enrollment_keys_strict_v1(entries: Seq<EnrollmentV1>) -> bool {
    forall|i: int, j: int| 0 <= i < j < entries.len()
        ==> enrollment_key_less_v1((#[trigger] entries[i]).key, (#[trigger] entries[j]).key)
}

proof fn enrollment_roster_order_v1(context: u64, entries: Seq<EnrollmentV1>, i: int, j: int)
    requires 0 <= i < j < entries.len(),
        forall|a: int| 0 <= a < entries.len() ==> #[trigger] enrollment_entry_admitted_v1(context, entries, a),
    ensures enrollment_key_less_v1(entries[i].key, entries[j].key),
    decreases j - i,
{
    assert(enrollment_entry_admitted_v1(context, entries, j));
    if i + 1 < j { enrollment_roster_order_v1(context, entries, i, j - 1); }
}

spec fn enrollment_header_ready_v1(context: u64, capacity: usize, free_len: usize,
    entries: Seq<EnrollmentV1>, output: Seq<Option<AllocationReferenceV1>>) -> bool {
    &&& entries.len() <= capacity
    &&& output.len() == entries.len()
    &&& forall|i: int| 0 <= i < output.len() ==> (#[trigger] output[i]).is_none()
    &&& forall|i: int| 0 <= i < entries.len() ==> enrollment_entry_error_v1(context, #[trigger] entries[i]).is_none()
    &&& enrollment_keys_strict_v1(entries)
    &&& enrollment_keys_sorted_v1(entries)
    &&& entries.len() > 0 ==> free_len <= capacity
}

proof fn enrollment_header_success_v1(context: u64, capacity: usize, free_len: usize,
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

spec fn enrollment_plan_output_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>)
    -> Seq<Option<AllocationReferenceV1>>
    recommends entries.len() <= journal.allocation_free@.len(),
{
    Seq::new(entries.len(), |i: int| Some(AllocationReferenceV1 {
        slot: journal.allocation_free@[journal.allocation_free@.len() - 1 - i], key: entries[i].key,
    }))
}

fn enrollment_fill_plan_exec_v1(journal: &JournalContentsV1, entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>])
    requires entries.len() <= journal.allocation_free@.len(), old(output).len() == entries.len(),
    ensures final(output)@ == enrollment_plan_output_v1(*journal, entries@),
{
    enrollment_fill_plan_body!(verus_exec_expr, journal, entries, output, index, [
        invariant index <= entries.len() <= journal.allocation_free@.len(), output.len() == entries.len(),
            forall|i: int| 0 <= i < index ==> (#[trigger] output@[i]) == enrollment_plan_output_v1(*journal, entries@)[i],
        decreases entries.len() - index,
    ], [proof { assert(output@ =~= enrollment_plan_output_v1(*journal, entries@)); }])
}

fn enrollment_clear_output_exec_v1(output: &mut [Option<AllocationReferenceV1>])
    ensures final(output)@ == Seq::new(old(output)@.len(), |i: int| None::<AllocationReferenceV1>),
{
    enrollment_clear_output_body!(verus_exec_expr, output, index, [
        invariant index <= output.len(), output@.len() == old(output)@.len(),
            forall|i: int| 0 <= i < index ==> (#[trigger] output@[i]).is_none(),
        decreases output.len() - index,
    ], [proof { assert(output@ =~= Seq::new(old(output)@.len(), |i: int| None::<AllocationReferenceV1>)); }])
}

proof fn enrollment_permutation_contains_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>, value: Option<AllocationReferenceV1>)
    requires before.to_multiset() == after.to_multiset(),
    ensures before.contains(value) == after.contains(value),
{
    vstd::seq_lib::to_multiset_contains(before, value);
    vstd::seq_lib::to_multiset_contains(after, value);
}

proof fn enrollment_permutation_unique_v1(before: Seq<Option<AllocationReferenceV1>>,
    after: Seq<Option<AllocationReferenceV1>>)
    requires before.to_multiset() == after.to_multiset(), before.no_duplicates(),
    ensures after.no_duplicates(),
{
    before.lemma_multiset_has_no_duplicates();
    after.lemma_multiset_has_no_duplicates_conv();
}

spec fn enrollment_slots_distinct_v1(values: Seq<Option<AllocationReferenceV1>>) -> bool {
    forall|i: int, j: int| 0 <= i < j < values.len()
        ==> (#[trigger] values[i]).unwrap().slot != (#[trigger] values[j]).unwrap().slot
}

#[verifier::spinoff_prover]
proof fn enrollment_permutation_distinct_slots_v1(before: Seq<Option<AllocationReferenceV1>>,
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

proof fn enrollment_permutation_slot_set_v1(before: Seq<Option<AllocationReferenceV1>>,
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

fn enrollment_duplicate_slots_exec_v1(values: &[Option<AllocationReferenceV1>]) -> (duplicate: bool)
    requires enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
    ensures duplicate == !enrollment_slots_distinct_v1(values@),
{
    enrollment_duplicate_slots_body!(verus_exec_expr, values, index, [
        invariant 1 <= index <= values.len(), enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
            forall|i: int| 0 < i < index ==> (#[trigger] values@[i - 1]).unwrap().slot != (#[trigger] values@[i]).unwrap().slot,
        decreases values.len() - index,
    ], [proof {
        assert forall|i: int, j: int| 0 <= i < j < values@.len() implies
            (#[trigger] values@[i]).unwrap().slot != (#[trigger] values@[j]).unwrap().slot by {
            if i < j - 1 { assert(values@[i].unwrap().slot <= values@[j - 1].unwrap().slot); }
            assert(values@[j - 1].unwrap().slot <= values@[j].unwrap().slot);
        }
    }])
}

spec fn enrollment_replay_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>) -> bool {
    exists|a: int, i: int| 0 <= a < journal.allocations@.len() && 0 <= i < entries.len()
        && (#[trigger] journal.allocations@[a]).is_some()
        && journal.allocations@[a].unwrap().key == (#[trigger] entries[i]).key
}

fn enrollment_replay_exec_v1(journal: &JournalContentsV1, entries: &[EnrollmentV1]) -> (replay: bool)
    requires enrollment_keys_sorted_v1(entries@),
    ensures replay == enrollment_replay_v1(*journal, entries@),
{
    enrollment_replay_body!(verus_exec_expr, journal, entries, index, [
        invariant index <= journal.allocations@.len(), enrollment_keys_sorted_v1(entries@),
            forall|a: int, i: int| 0 <= a < index && 0 <= i < entries@.len()
                && (#[trigger] journal.allocations@[a]).is_some()
                ==> journal.allocations@[a].unwrap().key != (#[trigger] entries@[i]).key,
        decreases journal.allocations.len() - index,
    ])
}

spec fn enrollment_selected_vacant_v1(journal: JournalContentsV1, count: nat) -> bool {
    forall|i: int| journal.allocation_free@.len() - count <= i < journal.allocation_free@.len() ==> {
        &&& (#[trigger] journal.allocation_free@[i]) < journal.allocations@.len()
        &&& journal.allocations@[journal.allocation_free@[i] as int].is_none()
    }
}

fn enrollment_selected_vacant_exec_v1(journal: &JournalContentsV1, remaining: usize) -> (vacant: bool)
    requires remaining <= journal.allocation_free@.len(),
    ensures vacant == enrollment_selected_vacant_v1(*journal, (journal.allocation_free@.len() - remaining) as nat),
{
    enrollment_selected_vacant_body!(verus_exec_expr, journal, remaining, index, [
        invariant remaining <= index <= journal.allocation_free@.len(),
            forall|i: int| remaining <= i < index ==> {
                &&& (#[trigger] journal.allocation_free@[i]) < journal.allocations@.len()
                &&& journal.allocations@[journal.allocation_free@[i] as int].is_none()
            },
        decreases journal.allocation_free.len() - index,
    ])
}

spec fn enrollment_retained_clear_v1(journal: JournalContentsV1,
    values: Seq<Option<AllocationReferenceV1>>, remaining: usize) -> bool {
    forall|a: int, i: int| 0 <= a < remaining && 0 <= i < values.len()
        ==> (#[trigger] journal.allocation_free@[a]) != (#[trigger] values[i]).unwrap().slot
}

fn enrollment_retained_clear_exec_v1(journal: &JournalContentsV1,
    values: &[Option<AllocationReferenceV1>], remaining: usize) -> (clear: bool)
    requires remaining <= journal.allocation_free@.len(), enrollment_all_some_v1(values@),
        enrollment_sort_frontier_v1(values@, 0),
    ensures clear == enrollment_retained_clear_v1(*journal, values@, remaining),
{
    enrollment_retained_clear_body!(verus_exec_expr, journal, values, remaining, index, [
        invariant index <= remaining <= journal.allocation_free@.len(),
            enrollment_all_some_v1(values@), enrollment_sort_frontier_v1(values@, 0),
            forall|a: int, i: int| 0 <= a < index && 0 <= i < values@.len()
                ==> (#[trigger] journal.allocation_free@[a]) != (#[trigger] values@[i]).unwrap().slot,
        decreases remaining - index,
    ])
}

proof fn enrollment_retained_permutation_v1(journal: JournalContentsV1,
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

proof fn enrollment_plan_unique_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>)
    requires entries.len() <= journal.allocation_free@.len(), enrollment_keys_strict_v1(entries),
    ensures enrollment_all_some_v1(enrollment_plan_output_v1(journal, entries)),
        enrollment_plan_output_v1(journal, entries).no_duplicates(),
{
    let values = enrollment_plan_output_v1(journal, entries);
    assert forall|i: int, j: int| 0 <= i < j < values.len() implies values[i] != values[j] by {
        assert(enrollment_key_less_v1(entries[i].key, entries[j].key));
    }
}

spec fn enrollment_decision_v1(journal: JournalContentsV1, entries: Seq<EnrollmentV1>,
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

spec fn enrollment_execution_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
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
fn enrollment_journal_exec_v1(contents: &mut JournalContentsV1, entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>]) -> (result: Result<(), EnrollmentErrorV1>)
    ensures enrollment_execution_relation_v1(*old(contents), *final(contents), entries@, old(output)@, final(output)@, result),
{
    let ghost before = *contents;
    let ghost original = output@;
    enrollment_journal_body!(verus_exec_expr, contents, entries, output, remaining, [
        proof { enrollment_header_success_v1(contents.context_generation, contents.allocation_capacity,
            contents.allocation_free.len(), entries@, output@); }
    ], [
        proof {
            assert(output@ =~= enrollment_plan_output_v1(before, entries@));
            assert(contents.allocation_free@ =~= before.allocation_free@.subrange(0, before.allocation_free@.len() as int));
        }
    ], [
        let ghost plan = output@;
        proof { enrollment_plan_unique_v1(before, entries@); }
    ], [
        proof {
            enrollment_permutation_distinct_slots_v1(plan, output@);
            enrollment_retained_permutation_v1(before, plan, output@, remaining);
        }
    ], [
        proof { assert(output@ =~= original); }
    ], [
        proof { assert(enrollment_journal_output_bound_v1(before, entries@, output@, remaining)); }
    ])
}

}

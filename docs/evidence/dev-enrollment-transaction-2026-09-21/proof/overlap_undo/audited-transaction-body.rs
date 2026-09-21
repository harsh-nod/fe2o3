use vstd::prelude::*;

verus! {

pub open spec fn enrollment_stage_bound_v1(before: Seq<Option<AllocationEntryV1>>,
    entries: Seq<EnrollmentV1>, plan: Seq<Option<AllocationReferenceV1>>, count: nat) -> bool {
    &&& count <= entries.len() == plan.len()
    &&& forall|i: int| 0 <= i < plan.len() ==> {
        &&& (#[trigger] plan[i]).is_some()
        &&& plan[i].unwrap().slot < before.len()
        &&& before[plan[i].unwrap().slot as int].is_none()
    }
    &&& forall|i: int, j: int| 0 <= i < j < count
        ==> (#[trigger] plan[i]).unwrap().slot != (#[trigger] plan[j]).unwrap().slot
}

pub proof fn enrollment_stage_at_v1(before: Seq<Option<AllocationEntryV1>>,
    entries: Seq<EnrollmentV1>, plan: Seq<Option<AllocationReferenceV1>>, count: nat, slot: int)
    requires enrollment_stage_bound_v1(before, entries, plan, count), 0 <= slot < before.len(),
    ensures enrollment_prefix_v1(before, entries, plan, count).len() == before.len(),
        (forall|i: int| 0 <= i < count ==> (#[trigger] plan[i]).unwrap().slot != slot)
            ==> enrollment_prefix_v1(before, entries, plan, count)[slot] == before[slot],
        forall|i: int| 0 <= i < count && (#[trigger] plan[i]).unwrap().slot == slot
            ==> enrollment_prefix_v1(before, entries, plan, count)[slot] == Some(enrollment_value_v1(entries[i])),
    decreases count,
{
    if count > 0 {
        enrollment_stage_at_v1(before, entries, plan, (count - 1) as nat, slot);
        assert forall|i: int| 0 <= i < count && (#[trigger] plan[i]).unwrap().slot == slot
            implies enrollment_prefix_v1(before, entries, plan, count)[slot] == Some(enrollment_value_v1(entries[i])) by {
            if i < count - 1 { assert(plan[i].unwrap().slot != plan[count - 1].unwrap().slot); }
        }
    }
}

pub proof fn enrollment_stage_pop_v1(before: Seq<Option<AllocationEntryV1>>,
    entries: Seq<EnrollmentV1>, plan: Seq<Option<AllocationReferenceV1>>, count: nat)
    requires enrollment_stage_bound_v1(before, entries, plan, count), count > 0,
    ensures enrollment_prefix_v1(before, entries, plan, count).update(plan[count - 1].unwrap().slot as int, None)
        == enrollment_prefix_v1(before, entries, plan, (count - 1) as nat),
{
    let slot = plan[count - 1].unwrap().slot as int;
    enrollment_stage_at_v1(before, entries, plan, (count - 1) as nat, slot);
    assert forall|i: int| 0 <= i < count - 1 implies (#[trigger] plan[i]).unwrap().slot != slot by {
        assert(plan[i].unwrap().slot != plan[count - 1].unwrap().slot);
    }
    assert(enrollment_prefix_v1(before, entries, plan, (count - 1) as nat)[slot].is_none());
    assert(enrollment_prefix_v1(before, entries, plan, count).update(slot, None)
        =~= enrollment_prefix_v1(before, entries, plan, (count - 1) as nat));
}

pub fn enrollment_stage_exec_v1(allocations: &mut Vec<Option<AllocationEntryV1>>, entries: &[EnrollmentV1],
    plan: &[Option<AllocationReferenceV1>]) -> (installed: usize)
    requires enrollment_stage_bound_v1(old(allocations)@, entries@, plan@, 0),
    ensures enrollment_stage_bound_v1(old(allocations)@, entries@, plan@, installed as nat),
        final(allocations)@ == enrollment_prefix_v1(old(allocations)@, entries@, plan@, installed as nat),
        final(allocations)@.len() == old(allocations)@.len(),
        (installed == entries.len()) == enrollment_slots_distinct_v1(plan@),
{
    let ghost before = allocations@;
    let mut installed = 0usize;
    while installed < entries.len()
        invariant installed <= entries.len(), before == old(allocations)@,
            enrollment_stage_bound_v1(before, entries@, plan@, installed as nat),
            allocations@ == enrollment_prefix_v1(before, entries@, plan@, installed as nat),
            allocations@.len() == before.len(),
        decreases entries.len() - installed,
    {
        let slot = plan[installed].unwrap().slot;
        proof { enrollment_stage_at_v1(before, entries@, plan@, installed as nat, slot as int); }
        if allocations[slot].is_some() {
            proof {
                assert(exists|i: int| 0 <= i < installed && (#[trigger] plan@[i]).unwrap().slot == slot);
                let i = choose|i: int| 0 <= i < installed && plan@[i].unwrap().slot == slot;
                assert(plan@[i].unwrap().slot == plan@[installed as int].unwrap().slot);
            }
            return installed;
        }
        proof {
            assert forall|i: int| 0 <= i < installed implies (#[trigger] plan@[i]).unwrap().slot != slot by {
                if plan@[i].unwrap().slot == slot {
                    assert(allocations@[slot as int] == Some(enrollment_value_v1(entries@[i])));
                }
            }
        }
        let entry = entries[installed];
        allocations.set(slot, Some(AllocationEntryV1 { key: entry.key, device: entry.device,
            byte_extent: entry.byte_extent, attempt_epoch: 0, content_lineage: 0, pending_member: None }));
        installed += 1;
    }
    installed
}

pub fn enrollment_unstage_exec_v1(allocations: &mut Vec<Option<AllocationEntryV1>>, entries: &[EnrollmentV1],
    plan: &[Option<AllocationReferenceV1>], installed: usize, Ghost(before): Ghost<Seq<Option<AllocationEntryV1>>>)
    requires enrollment_stage_bound_v1(before, entries@, plan@, installed as nat),
        old(allocations)@ == enrollment_prefix_v1(before, entries@, plan@, installed as nat),
        old(allocations)@.len() == before.len(),
    ensures final(allocations)@ == before,
{
    let mut count = installed;
    while count > 0
        invariant count <= installed,
            enrollment_stage_bound_v1(before, entries@, plan@, installed as nat),
            allocations@ == enrollment_prefix_v1(before, entries@, plan@, count as nat),
            allocations@.len() == before.len(),
        decreases count,
    {
        proof { enrollment_stage_pop_v1(before, entries@, plan@, count as nat); }
        count -= 1;
        allocations.set(plan[count].unwrap().slot, None);
    }
}

pub open spec fn enrollment_stage_member_v1(allocations: Seq<Option<AllocationEntryV1>>,
    entries: Seq<EnrollmentV1>, slot: usize) -> bool {
    slot < allocations.len() && allocations[slot as int].is_some()
        && exists|i: int| 0 <= i < entries.len() && (#[trigger] entries[i]).key == allocations[slot as int].unwrap().key
}

pub proof fn enrollment_stage_membership_v1(before: JournalContentsV1, entries: Seq<EnrollmentV1>,
    plan: Seq<Option<AllocationReferenceV1>>, after: Seq<Option<AllocationEntryV1>>, slot: usize)
    requires enrollment_stage_bound_v1(before.allocations@, entries, plan, entries.len()),
        !enrollment_replay_v1(before, entries),
        after == enrollment_prefix_v1(before.allocations@, entries, plan, entries.len()),
        after.len() == before.allocations@.len(),
    ensures enrollment_stage_member_v1(after, entries, slot)
        == (exists|i: int| 0 <= i < plan.len() && (#[trigger] plan[i]).unwrap().slot == slot),
{
    if slot < before.allocations@.len() {
        enrollment_stage_at_v1(before.allocations@, entries, plan, entries.len(), slot as int);
        if exists|i: int| 0 <= i < plan.len() && plan[i].unwrap().slot == slot {
            let i = choose|i: int| 0 <= i < plan.len() && plan[i].unwrap().slot == slot;
            assert(after[slot as int] == Some(enrollment_value_v1(entries[i])));
            assert(entries[i].key == after[slot as int].unwrap().key);
        } else {
            assert(after[slot as int] == before.allocations@[slot as int]);
            assert forall|i: int| 0 <= i < entries.len() && after[slot as int].is_some() implies
                (#[trigger] entries[i]).key != after[slot as int].unwrap().key by {
                assert(before.allocations@[slot as int].unwrap().key != entries[i].key);
            }
        }
    }
}

pub fn enrollment_staged_retained_clear_exec_v1(contents: &JournalContentsV1, entries: &[EnrollmentV1],
    plan: &[Option<AllocationReferenceV1>], remaining: usize, Ghost(before): Ghost<JournalContentsV1>) -> (clear: bool)
    requires enrollment_stage_bound_v1(before.allocations@, entries@, plan@, entries@.len()),
        !enrollment_replay_v1(before, entries@), enrollment_keys_sorted_v1(entries@),
        contents.allocations@ == enrollment_prefix_v1(before.allocations@, entries@, plan@, entries@.len()),
        contents.allocations@.len() == before.allocations@.len(),
        contents.allocation_free@ == before.allocation_free@, remaining <= before.allocation_free@.len(),
    ensures clear == enrollment_retained_clear_v1(before, plan@, remaining),
{
    let mut index = 0usize;
    while index < remaining
        invariant index <= remaining <= before.allocation_free@.len(),
            enrollment_stage_bound_v1(before.allocations@, entries@, plan@, entries@.len()),
            !enrollment_replay_v1(before, entries@), enrollment_keys_sorted_v1(entries@),
            contents.allocations@ == enrollment_prefix_v1(before.allocations@, entries@, plan@, entries@.len()),
            contents.allocations@.len() == before.allocations@.len(), contents.allocation_free@ == before.allocation_free@,
            forall|a: int, i: int| 0 <= a < index && 0 <= i < plan@.len()
                ==> (#[trigger] before.allocation_free@[a]) != (#[trigger] plan@[i]).unwrap().slot,
        decreases remaining - index,
    {
        let slot = contents.allocation_free[index];
        proof { enrollment_stage_membership_v1(before, entries@, plan@, contents.allocations@, slot); }
        if slot < contents.allocations.len() {
            if let Some(entry) = contents.allocations[slot] {
                if enrollment_contains_key_exec_v1(entries, entry.key) { return false; }
            }
        }
        index += 1;
    }
    true
}

// Vec::set specifies its sequence view, not the identity of the opaque Vec.
// Keep the historical stronger relation unchanged; name this boundary explicitly.
pub open spec fn enrollment_transaction_relation_v1(before: JournalContentsV1, after: JournalContentsV1,
    entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>,
    output: Seq<Option<AllocationReferenceV1>>, result: Result<(), EnrollmentErrorV1>) -> bool {
    &&& result == enrollment_decision_v1(before, entries, original)
    &&& match result {
        Err(_) => {
            &&& enrollment_untouched_journal_v1(before, after)
            &&& after.writers == before.writers && after.free == before.free
            &&& after.members == before.members && after.member_free == before.member_free
            &&& after.scratch == before.scratch && after.allocation_free == before.allocation_free
            &&& after.allocations@ == before.allocations@
            &&& output == original
        },
        Ok(()) => enrollment_execution_relation_v1(before, after, entries, original, output, result),
    }
}

pub proof fn enrollment_historical_implies_transaction_v1(before: JournalContentsV1, after: JournalContentsV1,
    entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>,
    output: Seq<Option<AllocationReferenceV1>>, result: Result<(), EnrollmentErrorV1>)
    requires enrollment_execution_relation_v1(before, after, entries, original, output, result),
    ensures enrollment_transaction_relation_v1(before, after, entries, original, output, result),
{}

#[verifier::spinoff_prover]
pub fn enrollment_transaction_exec_v1(contents: &mut JournalContentsV1, entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>]) -> (result: Result<(), EnrollmentErrorV1>)
    ensures enrollment_transaction_relation_v1(*old(contents), *final(contents), entries@, old(output)@, final(output)@, result),
{
    let ghost before = *contents;
    let ghost original = output@;
    if let Err(error) = enrollment_header_exec_v1(contents.context_generation, contents.allocation_capacity,
        contents.allocation_free.len(), entries, output) { return Err(error); }
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
    proof { assert(enrollment_stage_bound_v1(before.allocations@, entries@, output@, 0)); }
    let installed = enrollment_stage_exec_v1(&mut contents.allocations, entries, output);
    if installed < entries.len() {
        enrollment_unstage_exec_v1(&mut contents.allocations, entries, output, installed, Ghost(before.allocations@));
        enrollment_clear_output_exec_v1(output);
        proof { assert(output@ =~= original); }
        return Err(EnrollmentErrorV1::InvalidState);
    }
    if !enrollment_staged_retained_clear_exec_v1(contents, entries, output, remaining, Ghost(before)) {
        enrollment_clear_output_exec_v1(output);
        proof { assert(output@ =~= original); }
        return Err(EnrollmentErrorV1::InvalidState);
    }
    contents.allocation_free.truncate(remaining);
    Ok(())
}

#[verifier::spinoff_prover]
pub fn enrollment_transaction_issued_exec_v1(contents: &mut ProducerReadContentsV1, entries: &[EnrollmentV1],
    output: &mut [Option<AllocationReferenceV1>], Ghost(storage): Ghost<StorageCapacitiesV1>,
    Ghost(history): Ghost<Seq<WriterReferenceV1>>) -> (result: Result<(), EnrollmentErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures enrollment_transaction_relation_v1(old(contents).stable.journal, final(contents).stable.journal,
            entries@, old(output)@, final(output)@, result),
        producer_storage_frame_v1(*old(contents), *final(contents)),
        issued_producer_v1(*final(contents), storage, history),
        forall|request: ProducerReadV1| producer_status_v1(old(contents).stable.journal, request).is_some()
            ==> #[trigger] producer_status_v1(final(contents).stable.journal, request)
                == producer_status_v1(old(contents).stable.journal, request),
{
    let ghost before = *contents;
    let ghost original = output@;
    let result = enrollment_transaction_exec_v1(&mut contents.stable.journal, entries, output);
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
        } else {
            let empty_entries = Seq::<EnrollmentV1>::empty();
            let empty_output = Seq::<Option<AllocationReferenceV1>>::empty();
            let remaining = before.stable.journal.allocation_free@.len() as usize;
            assert(contents.stable.journal.allocation_free@ =~=
                before.stable.journal.allocation_free@.subrange(0, remaining as int));
            enrollment_preserves_issued_producer_v1(before, *contents, empty_entries, empty_output,
                remaining, storage, history);
            assert forall|request: ProducerReadV1| producer_status_v1(before.stable.journal, request).is_some()
                implies #[trigger] producer_status_v1(contents.stable.journal, request)
                    == producer_status_v1(before.stable.journal, request) by {
                producer_status_enrollment_frame_v1(before.stable.journal, contents.stable.journal, request);
            }
        }
    }
    result
}

}

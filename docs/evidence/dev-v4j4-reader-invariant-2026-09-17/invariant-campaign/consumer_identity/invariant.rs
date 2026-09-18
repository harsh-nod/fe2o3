// V4-J4 development: reader arena preservation; native refinement remains separate.
include!("context_read_commit_v1.rs");

verus! {

pub open spec fn lease_weight_v1(entry: Option<ReadLeaseV1>, allocation: usize) -> nat {
    if entry.is_some() && entry.unwrap().request.allocation.slot == allocation { 1 } else { 0 }
}

pub open spec fn lease_count_v1(leases: Seq<Option<ReadLeaseV1>>, allocation: usize) -> nat
    decreases leases.len(),
{
    if leases.len() == 0 { 0 }
    else { lease_count_v1(leases.drop_last(), allocation) + lease_weight_v1(leases.last(), allocation) }
}

pub proof fn lease_count_bound_v1(leases: Seq<Option<ReadLeaseV1>>, allocation: usize)
    ensures lease_count_v1(leases, allocation) <= leases.len(),
    decreases leases.len(),
{
    if leases.len() > 0 { lease_count_bound_v1(leases.drop_last(), allocation); }
}

pub proof fn lease_count_update_v1(
    leases: Seq<Option<ReadLeaseV1>>, slot: int, entry: Option<ReadLeaseV1>, allocation: usize,
)
    requires 0 <= slot < leases.len(),
    ensures lease_count_v1(leases.update(slot, entry), allocation) + lease_weight_v1(leases[slot], allocation)
        == lease_count_v1(leases, allocation) + lease_weight_v1(entry, allocation),
    decreases leases.len(),
{
    if slot < leases.len() - 1 {
        assert(leases.update(slot, entry).drop_last() =~= leases.drop_last().update(slot, entry));
        lease_count_update_v1(leases.drop_last(), slot, entry, allocation);
    } else {
        assert(leases.update(slot, entry).drop_last() =~= leases.drop_last());
    }
}

pub proof fn lease_count_zero_v1(leases: Seq<Option<ReadLeaseV1>>, allocation: usize)
    ensures (lease_count_v1(leases, allocation) == 0) <==>
        (forall|s: int| 0 <= s < leases.len() ==> lease_weight_v1(#[trigger] leases[s], allocation) == 0),
    decreases leases.len(),
{
    if leases.len() > 0 {
        lease_count_zero_v1(leases.drop_last(), allocation);
        if lease_count_v1(leases, allocation) == 0 {
            assert forall|s: int| 0 <= s < leases.len() implies lease_weight_v1(#[trigger] leases[s], allocation) == 0 by {
                if s < leases.len() - 1 { assert(leases.drop_last()[s] == leases[s]); }
            }
        } else if forall|s: int| 0 <= s < leases.len() ==> lease_weight_v1(#[trigger] leases[s], allocation) == 0 {
            assert forall|s: int| 0 <= s < leases.drop_last().len() implies
                lease_weight_v1(#[trigger] leases.drop_last()[s], allocation) == 0 by {
                assert(leases[s] == leases.drop_last()[s]);
            }
            assert(lease_weight_v1(leases[leases.len() - 1], allocation) == 0);
        }
    }
}

pub open spec fn reader_arena_v1(
    journal: JournalContentsV1, leases: Seq<Option<ReadLeaseV1>>, free: Seq<usize>, readers: Seq<usize>, next: u64,
) -> bool {
    &&& 0 < leases.len() <= 1_048_576
    &&& readers.len() == journal.allocations@.len()
    &&& 0 < readers.len() <= 1_048_576
    &&& 0 < next
    &&& free.len() <= leases.len()
    &&& free.no_duplicates()
    &&& forall|i: int| 0 <= i < free.len() ==> free[i] < leases.len() && leases[free[i] as int].is_none()
    &&& forall|s: int| 0 <= s < leases.len() ==> leases[s].is_none() == free.contains(s as usize)
    &&& forall|a: int| 0 <= a < readers.len() ==> readers[a] == lease_count_v1(leases, a as usize)
    &&& forall|s: int| 0 <= s < leases.len() && (#[trigger] leases[s]).is_some() ==> {
        let entry = leases[s].unwrap();
        &&& entry.reference.slot == s
        &&& entry.reference.consumer.context_generation == journal.context_generation
        &&& issuable_id_v1(entry.reference.consumer.local)
        &&& 0 < entry.reference.incarnation < next
        &&& entry.request.allocation.slot < readers.len()
        &&& read_decision_v1(journal, entry.request) == Ok(())
    }
    &&& forall|s: int, t: int| 0 <= s < t < leases.len()
        && (#[trigger] leases[s]).is_some() && (#[trigger] leases[t]).is_some()
        ==> leases[s].unwrap().reference.incarnation != leases[t].unwrap().reference.incarnation
}

pub open spec fn reader_invariant_v1(contents: ReadContentsV1) -> bool {
    reader_arena_v1(contents.journal, contents.leases@, contents.free_reads@, contents.readers@, contents.next_incarnation)
}

pub proof fn reader_constructor_invariant_v1(
    context: u64, allocations: usize, writers: usize, reads: usize, contents: ReadContentsV1,
)
    requires reader_constructor_relation_v1(context, allocations, writers, reads, Ok(contents)),
    ensures reader_invariant_v1(contents),
{
    let free = contents.free_reads@;
    assert forall|s: int| 0 <= s < contents.leases@.len() implies #[trigger] free.contains(s as usize) by {
        assert(free[reads - 1 - s] == s);
    }
    assert forall|a: int| 0 <= a < contents.readers@.len() implies
        contents.readers@[a] == lease_count_v1(contents.leases@, a as usize) by {
        lease_count_zero_v1(contents.leases@, a as usize);
    }
}

pub proof fn reader_selected_unique_v1(contents: ReadContentsV1, count: nat)
    requires reader_invariant_v1(contents), count <= contents.free_reads@.len(),
    ensures selected_free_unique_v1(contents, count),
{}

// Sequence algebra uses a prefix watermark; the executable loop advances its
// actual watermark once, after installing the complete batch.
pub proof fn arena_insert_v1(
    journal: JournalContentsV1, leases: Seq<Option<ReadLeaseV1>>, free: Seq<usize>, readers: Seq<usize>, next: u64,
    entry: ReadLeaseV1,
)
    requires reader_arena_v1(journal, leases, free, readers, next), free.len() > 0,
        entry.reference.slot == free.last(), entry.reference.incarnation == next, next < u64::MAX,
        entry.reference.consumer.context_generation == journal.context_generation,
        issuable_id_v1(entry.reference.consumer.local),
        entry.request.allocation.slot < readers.len(), read_decision_v1(journal, entry.request) == Ok(()),
        readers[entry.request.allocation.slot as int] < usize::MAX,
    ensures reader_arena_v1(journal, leases.update(entry.reference.slot as int, Some(entry)), free.drop_last(),
        readers.update(entry.request.allocation.slot as int, (readers[entry.request.allocation.slot as int] + 1) as usize),
        (next + 1) as u64),
{
    let slot = entry.reference.slot as int;
    let after = leases.update(slot, Some(entry));
    assert(!free.drop_last().contains(slot as usize)) by {
        if free.drop_last().contains(slot as usize) {
            let i = choose|i: int| 0 <= i < free.drop_last().len() && free.drop_last()[i] == slot;
            assert(free[i] == free[free.len() - 1]);
        }
    }
    assert forall|s: int| 0 <= s < after.len() implies after[s].is_none() == free.drop_last().contains(s as usize) by {
        if s != slot && free.contains(s as usize) {
            let i = choose|i: int| 0 <= i < free.len() && free[i] == s;
            assert(i < free.len() - 1);
            assert(free.drop_last()[i] == s);
        }
    }
    assert forall|a: int| 0 <= a < readers.len() implies
        readers.update(entry.request.allocation.slot as int, (readers[entry.request.allocation.slot as int] + 1) as usize)[a]
            == #[trigger] lease_count_v1(after, a as usize) by {
        lease_count_update_v1(leases, slot, Some(entry), a as usize);
        assert(readers[a] == lease_count_v1(leases, a as usize));
    }
}

pub proof fn arena_remove_v1(
    journal: JournalContentsV1, leases: Seq<Option<ReadLeaseV1>>, free: Seq<usize>, readers: Seq<usize>, next: u64, slot: usize,
)
    requires reader_arena_v1(journal, leases, free, readers, next), slot < leases.len(), leases[slot as int].is_some(),
        free.len() < leases.len(), readers[leases[slot as int].unwrap().request.allocation.slot as int] > 0,
    ensures reader_arena_v1(journal, leases.update(slot as int, None), free.push(slot),
        readers.update(leases[slot as int].unwrap().request.allocation.slot as int,
            (readers[leases[slot as int].unwrap().request.allocation.slot as int] - 1) as usize), next),
{
    let entry = leases[slot as int].unwrap();
    let after = leases.update(slot as int, None);
    assert(!free.contains(slot));
    assert forall|s: int| 0 <= s < after.len() implies after[s].is_none() == free.push(slot).contains(s as usize) by {
        if s == slot { assert(free.push(slot)[free.len() as int] == s); }
        else if leases[s].is_none() {
            assert(free.contains(s as usize));
            let i = choose|i: int| 0 <= i < free.len() && free[i] == s;
            assert(free.push(slot)[i] == s);
        }
        if free.push(slot).contains(s as usize) && s != slot {
            let i = choose|i: int| 0 <= i < free.push(slot).len() && free.push(slot)[i] == s;
            assert(i < free.len());
            assert(free[i] == s);
        }
    }
    assert forall|a: int| 0 <= a < readers.len() implies
        readers.update(entry.request.allocation.slot as int, (readers[entry.request.allocation.slot as int] - 1) as usize)[a]
            == #[trigger] lease_count_v1(after, a as usize) by {
        lease_count_update_v1(leases, slot as int, None, a as usize);
        assert(readers[a] == lease_count_v1(leases, a as usize));
    }
}

pub open spec fn acquired_readers_v1(before: ReadContentsV1, requests: Seq<AllocationReadV1>, count: nat) -> Seq<usize> {
    Seq::new(before.readers@.len(), |a: int|
        (before.readers@[a] + read_slot_count_v1(requests.take(count as int), a as usize)) as usize)
}

pub proof fn acquire_arena_prefix_v1(before: ReadContentsV1, consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, count: nat)
    requires reader_invariant_v1(before), acquire_commit_ready_v1(before, requests), count <= requests.len(),
        consumer.context_generation == before.journal.context_generation, issuable_id_v1(consumer.local),
        forall|i: int| 0 <= i < requests.len() ==> read_decision_v1(before.journal, #[trigger] requests[i]) == Ok(()),
    ensures reader_arena_v1(before.journal, acquired_leases_v1(before, consumer, requests, count),
        before.free_reads@.take(before.free_reads@.len() - count), acquired_readers_v1(before, requests, count),
        (before.next_incarnation + count) as u64),
    decreases count,
{
    if count == 0 {
        assert(requests.take(0) =~= Seq::<AllocationReadV1>::empty());
        assert(acquired_readers_v1(before, requests, 0) =~= before.readers@);
        assert(before.free_reads@.take(before.free_reads@.len() as int) =~= before.free_reads@);
    } else {
        let n = (count - 1) as nat;
        acquire_arena_prefix_v1(before, consumer, requests, n);
        let reference = acquired_reference_v1(before, consumer, n as int);
        let entry = ReadLeaseV1 { reference, request: requests[n as int] };
        let leases = acquired_leases_v1(before, consumer, requests, n);
        let free = before.free_reads@.take(before.free_reads@.len() - n);
        let readers = acquired_readers_v1(before, requests, n);
        assert(requests.take(count as int) =~= requests.take(n as int).push(requests[n as int]));
        read_slot_count_push_v1(requests.take(n as int), entry.request, entry.request.allocation.slot);
        arena_insert_v1(before.journal, leases, free, readers, (before.next_incarnation + n) as u64, entry);
        assert(free.drop_last() =~= before.free_reads@.take(before.free_reads@.len() - count));
        assert(acquired_readers_v1(before, requests, count) =~=
            readers.update(entry.request.allocation.slot as int, (readers[entry.request.allocation.slot as int] + 1) as usize)) by {
            assert forall|a: int| 0 <= a < readers.len() implies #[trigger] acquired_readers_v1(before, requests, count)[a]
                == readers.update(entry.request.allocation.slot as int, (readers[entry.request.allocation.slot as int] + 1) as usize)[a] by {
                read_slot_count_push_v1(requests.take(n as int), entry.request, a as usize);
            }
        }
    }
}

pub open spec fn released_readers_v1(before: ReadContentsV1, references: Seq<ReadReferenceV1>, count: nat) -> Seq<usize> {
    Seq::new(before.readers@.len(), |a: int|
        (before.readers@[a] - read_slot_count_v1(released_requests_v1(before, references).take(count as int), a as usize)) as usize)
}

pub proof fn release_arena_prefix_v1(before: ReadContentsV1, references: Seq<ReadReferenceV1>, count: nat)
    requires reader_invariant_v1(before), release_commit_ready_v1(before, references), count <= references.len(),
    ensures reader_arena_v1(before.journal, released_leases_v1(before, references, count),
        before.free_reads@ + Seq::new(count, |i: int| references[i].slot), released_readers_v1(before, references, count),
        before.next_incarnation),
    decreases count,
{
    if count == 0 {
        assert(released_requests_v1(before, references).take(0) =~= Seq::<AllocationReadV1>::empty());
        assert(released_readers_v1(before, references, 0) =~= before.readers@);
        assert(before.free_reads@ + Seq::new(0, |i: int| references[i].slot) =~= before.free_reads@);
    } else {
        let n = (count - 1) as nat;
        release_arena_prefix_v1(before, references, n);
        let slot = references[n as int].slot;
        released_leases_unselected_v1(before, references, n, slot as int);
        let entry = before.leases@[slot as int].unwrap();
        let leases = released_leases_v1(before, references, n);
        let free = before.free_reads@ + Seq::new(n, |i: int| references[i].slot);
        let readers = released_readers_v1(before, references, n);
        let requests = released_requests_v1(before, references);
        assert(requests.take(count as int) =~= requests.take(n as int).push(requests[n as int]));
        read_slot_count_push_v1(requests.take(n as int), entry.request, entry.request.allocation.slot);
        arena_remove_v1(before.journal, leases, free, readers, before.next_incarnation, slot);
        assert(free.push(slot) =~= before.free_reads@ + Seq::new(count, |i: int| references[i].slot));
        assert(released_readers_v1(before, references, count) =~=
            readers.update(entry.request.allocation.slot as int, (readers[entry.request.allocation.slot as int] - 1) as usize)) by {
            assert forall|a: int| 0 <= a < readers.len() implies #[trigger] released_readers_v1(before, references, count)[a]
                == readers.update(entry.request.allocation.slot as int, (readers[entry.request.allocation.slot as int] - 1) as usize)[a] by {
                read_slot_count_push_v1(requests.take(n as int), entry.request, a as usize);
            }
        }
    }
}

pub proof fn acquire_preserves_reader_invariant_v1(
    before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1, requests: Seq<AllocationReadV1>,
    output_before: Seq<Option<ReadReferenceV1>>, output_after: Seq<Option<ReadReferenceV1>>, result: Result<(), ReadErrorV1>,
)
    requires reader_invariant_v1(before), requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        acquire_execution_relation_v1(before, after, consumer, requests, output_before, output_after, result),
    ensures reader_invariant_v1(after), before.next_incarnation <= after.next_incarnation,
{
    if result.is_ok() {
        if let Ok(value) = result { assert(value =~= ()); }
        reader_selected_unique_v1(before, requests.len());
        acquire_preflight_implies_commit_ready_v1(before, consumer, requests, output_before);
        acquire_arena_prefix_v1(before, consumer, requests, requests.len());
        assert(requests.take(requests.len() as int) =~= requests);
        assert(after.readers@ =~= acquired_readers_v1(before, requests, requests.len()));
    }
}

pub proof fn release_preserves_reader_invariant_v1(
    before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1, references: Seq<ReadReferenceV1>,
    evidence_consumer: WriterKeyV1, observed_free_capacity: usize, result: Result<(), ReadErrorV1>,
)
    requires reader_invariant_v1(before), references.len() <= usize::MAX,
        release_execution_relation_v1(before, after, consumer, references, evidence_consumer, observed_free_capacity, result),
    ensures reader_invariant_v1(after), after.next_incarnation == before.next_incarnation,
{
    if result.is_ok() {
        if let Ok(value) = result { assert(value =~= ()); }
        release_preflight_implies_commit_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity);
        release_arena_prefix_v1(before, references, references.len());
        assert(released_requests_v1(before, references).take(references.len() as int) =~= released_requests_v1(before, references));
        assert(after.readers@ =~= released_readers_v1(before, references, references.len()));
    }
}

pub fn reader_constructor_verified_v1(context: u64, allocations: usize, writers: usize, reads: usize)
    -> (result: Result<ReadContentsV1, ReadErrorV1>)
    ensures reader_constructor_relation_v1(context, allocations, writers, reads, result),
        match result { Ok(contents) => reader_invariant_v1(contents), Err(_) => true },
{
    let result = reader_constructor_exec_v1(context, allocations, writers, reads);
    proof { if let Ok(contents) = &result { reader_constructor_invariant_v1(context, allocations, writers, reads, *contents); } }
    result
}

pub fn acquire_verified_v1(contents: &mut ReadContentsV1, consumer: WriterKeyV1,
    requests: &[AllocationReadV1], output: &mut Vec<Option<ReadReferenceV1>>,
) -> (result: Result<(), ReadErrorV1>)
    requires reader_invariant_v1(*old(contents)), requests@.len() <= u64::MAX,
    ensures reader_invariant_v1(*final(contents)), old(contents).next_incarnation <= final(contents).next_incarnation,
        acquire_execution_relation_v1(*old(contents), *final(contents), consumer, requests@, old(output)@, final(output)@, result),
{
    let ghost before = *contents;
    let ghost output_before = output@;
    let _count = requests.len();
    proof {
        if acquire_decision_v1(*contents, consumer, requests@, output@) == Ok(()) {
            reader_selected_unique_v1(*contents, requests@.len());
        }
    }
    let result = acquire_contents_exec_v1(contents, consumer, requests, output);
    proof { acquire_preserves_reader_invariant_v1(before, *contents, consumer, requests@, output_before, output@, result); }
    result
}

pub fn release_verified_v1(contents: &mut ReadContentsV1, consumer: WriterKeyV1,
    references: &[ReadReferenceV1], evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
) -> (result: Result<(), ReadErrorV1>)
    requires reader_invariant_v1(*old(contents)),
    ensures reader_invariant_v1(*final(contents)), final(contents).next_incarnation == old(contents).next_incarnation,
        release_execution_relation_v1(*old(contents), *final(contents), consumer, references@, evidence_consumer, observed_free_capacity, result),
{
    let ghost before = *contents;
    let _count = references.len();
    let result = release_contents_exec_v1(contents, consumer, references, evidence_consumer, observed_free_capacity);
    proof { release_preserves_reader_invariant_v1(before, *contents, consumer, references@, evidence_consumer, observed_free_capacity, result); }
    result
}

pub proof fn reader_count_consequences_v1(contents: ReadContentsV1, allocation: usize)
    requires reader_invariant_v1(contents), allocation < contents.readers@.len(),
    ensures contents.readers@[allocation as int] <= contents.leases@.len(),
        (contents.readers@[allocation as int] == 0) <==>
            (forall|s: int| 0 <= s < contents.leases@.len() ==> lease_weight_v1(#[trigger] contents.leases@[s], allocation) == 0),
        contents.readers@[allocation as int] > 0 ==> {
            &&& contents.journal.allocations@[allocation as int].is_some()
            &&& contents.journal.allocations@[allocation as int].unwrap().pending_member.is_none()
        },
{
    lease_count_bound_v1(contents.leases@, allocation);
    lease_count_zero_v1(contents.leases@, allocation);
    if contents.readers@[allocation as int] > 0 {
        let s = choose|s: int| 0 <= s < contents.leases@.len()
            && lease_weight_v1(contents.leases@[s], allocation) != 0;
        assert(read_decision_v1(contents.journal, contents.leases@[s].unwrap().request) == Ok(()));
    }
}

pub proof fn retained_reader_consequences_v1(contents: ReadContentsV1, slot: usize)
    requires reader_invariant_v1(contents), slot < contents.leases@.len(), contents.leases@[slot as int].is_some(),
    ensures lease_decision_v1(contents, contents.leases@[slot as int].unwrap().reference)
            == Ok(contents.leases@[slot as int].unwrap().request),
        unread_scan_v1(contents, seq![contents.leases@[slot as int].unwrap().request.allocation], 0)
            == Err(ReadErrorV1::AllocationBusy),
{
    let entry = contents.leases@[slot as int].unwrap();
    reader_count_consequences_v1(contents, entry.request.allocation.slot);
    assert(lease_weight_v1(contents.leases@[slot as int], entry.request.allocation.slot) == 1);
}

// A field-level premise for future base transitions, not their refinement proof.
pub proof fn reader_allocation_frame_preserves_v1(before: ReadContentsV1, after: ReadContentsV1)
    requires reader_invariant_v1(before),
        after.journal.context_generation == before.journal.context_generation,
        after.journal.allocations@.len() == before.journal.allocations@.len(),
        after.leases@ == before.leases@, after.free_reads@ == before.free_reads@,
        after.readers@ == before.readers@, after.next_incarnation == before.next_incarnation,
        forall|a: int| 0 <= a < before.readers@.len() && before.readers@[a] > 0
            ==> after.journal.allocations@[a] == before.journal.allocations@[a],
    ensures reader_invariant_v1(after),
{
    assert forall|s: int| 0 <= s < after.leases@.len() && (#[trigger] after.leases@[s]).is_some() implies {
        let entry = after.leases@[s].unwrap();
        &&& entry.reference.slot == s
        &&& entry.reference.consumer.context_generation == after.journal.context_generation
        &&& issuable_id_v1(entry.reference.consumer.local)
        &&& 0 < entry.reference.incarnation < after.next_incarnation
        &&& entry.request.allocation.slot < after.readers@.len()
        &&& read_decision_v1(after.journal, entry.request) == Ok(())
    } by {
        let entry = before.leases@[s].unwrap();
        let a = entry.request.allocation.slot;
        reader_count_consequences_v1(before, a);
        assert(lease_weight_v1(before.leases@[s], a) == 1);
        assert(before.readers@[a as int] > 0);
        assert(before.journal.allocations@[a as int].unwrap().pending_member.is_none());
    }
}

pub open spec fn reader_storage_frame_v1(before: ReadContentsV1, after: ReadContentsV1) -> bool {
    &&& after.leases@ == before.leases@
    &&& after.free_reads@ == before.free_reads@
    &&& after.readers@ == before.readers@
    &&& after.next_incarnation == before.next_incarnation
}

pub fn register_reader_journal_v1(contents: &mut ReadContentsV1, key: WriterKeyV1)
    -> (result: Result<WriterReferenceV1, JournalErrorV1>)
    requires reader_invariant_v1(*old(contents)),
    ensures reader_invariant_v1(*final(contents)), reader_storage_frame_v1(*old(contents), *final(contents)),
        issuance_contents_frame_v1(old(contents).journal, final(contents).journal),
        register_execution_relation_v1(old(contents).journal.context_generation, old(contents).journal.writer_capacity, key,
            old(contents).journal.writers@, old(contents).journal.free@, old(contents).journal.registration_watermark,
            old(contents).journal.reserved_count, final(contents).journal.writers@, final(contents).journal.free@,
            final(contents).journal.registration_watermark, final(contents).journal.reserved_count, result),
{
    let ghost before = *contents;
    let result = register_contents_exec_v1(&mut contents.journal, key);
    proof { reader_allocation_frame_preserves_v1(before, *contents); }
    result
}

pub fn abort_reader_journal_v1(contents: &mut ReadContentsV1, observed_free_capacity: usize, reference: WriterReferenceV1)
    -> (result: Result<(), JournalErrorV1>)
    requires reader_invariant_v1(*old(contents)),
    ensures reader_invariant_v1(*final(contents)), reader_storage_frame_v1(*old(contents), *final(contents)),
        issuance_contents_frame_v1(old(contents).journal, final(contents).journal),
        abort_execution_relation_v1(old(contents).journal.context_generation, old(contents).journal.writer_capacity,
            observed_free_capacity, reference, old(contents).journal.writers@, old(contents).journal.free@,
            old(contents).journal.registration_watermark, old(contents).journal.reserved_count,
            final(contents).journal.writers@, final(contents).journal.free@, final(contents).journal.registration_watermark,
            final(contents).journal.reserved_count, result),
{
    let ghost before = *contents;
    let result = abort_contents_exec_v1(&mut contents.journal, observed_free_capacity, reference);
    proof { reader_allocation_frame_preserves_v1(before, *contents); }
    result
}

pub open spec fn reader_epoch_step_v1(before: u64, after: u64, minted: Seq<ReadReferenceV1>) -> bool {
    &&& 0 < before
    &&& after == before + minted.len()
    &&& forall|i: int| 0 <= i < minted.len() ==> (#[trigger] minted[i]).incarnation == before + i
}

pub proof fn acquire_epoch_projection_v1(before: ReadContentsV1, after: ReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<AllocationReadV1>, output_before: Seq<Option<ReadReferenceV1>>, output_after: Seq<Option<ReadReferenceV1>>,
    result: Result<(), ReadErrorV1>,
)
    requires reader_invariant_v1(before), requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        acquire_execution_relation_v1(before, after, consumer, requests, output_before, output_after, result),
    ensures reader_epoch_step_v1(before.next_incarnation, after.next_incarnation,
        if result.is_ok() { Seq::new(output_after.len(), |i: int| output_after[i].unwrap()) } else { Seq::empty() }),
{
    if result.is_ok() {
        if let Ok(value) = result { assert(value =~= ()); }
        reader_selected_unique_v1(before, requests.len());
        acquire_preflight_implies_commit_ready_v1(before, consumer, requests, output_before);
    }
}

pub open spec fn reader_epoch_trace_v1(frontiers: Seq<u64>, minted: Seq<Seq<ReadReferenceV1>>) -> bool {
    &&& frontiers.len() == minted.len() + 1
    &&& frontiers[0] == 1
    &&& forall|i: int| 0 <= i < minted.len() ==> reader_epoch_step_v1(frontiers[i], frontiers[i + 1], #[trigger] minted[i])
}

pub proof fn reader_epoch_monotone_v1(frontiers: Seq<u64>, minted: Seq<Seq<ReadReferenceV1>>, first: nat, last: nat)
    requires reader_epoch_trace_v1(frontiers, minted), first <= last <= minted.len(),
    ensures frontiers[first as int] <= frontiers[last as int],
    decreases last - first,
{
    if first < last {
        reader_epoch_monotone_v1(frontiers, minted, first, (last - 1) as nat);
        assert(reader_epoch_step_v1(frontiers[last - 1], frontiers[last as int], minted[last - 1]));
    }
}

// This is a trace theorem: releases and journal-only steps append empty mint
// rosters; acquisition appends exactly its successful output roster.
pub proof fn reader_history_never_reissues_v1(frontiers: Seq<u64>, minted: Seq<Seq<ReadReferenceV1>>,
    first: nat, first_index: nat, last: nat, last_index: nat,
)
    requires reader_epoch_trace_v1(frontiers, minted), first < last < minted.len(),
        first_index < minted[first as int].len(), last_index < minted[last as int].len(),
    ensures 0 < minted[first as int][first_index as int].incarnation
        < minted[last as int][last_index as int].incarnation,
{
    reader_epoch_monotone_v1(frontiers, minted, first + 1, last);
}

pub enum ReaderStepV1 {
    Acquire { consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, output_before: Seq<Option<ReadReferenceV1>>,
        output_after: Seq<Option<ReadReferenceV1>>, result: Result<(), ReadErrorV1> },
    Release { consumer: WriterKeyV1, references: Seq<ReadReferenceV1>, evidence_consumer: WriterKeyV1,
        observed_free_capacity: usize, result: Result<(), ReadErrorV1> },
    Register { key: WriterKeyV1, register_result: Result<WriterReferenceV1, JournalErrorV1> },
    Abort { reference: WriterReferenceV1, observed_free_capacity: usize, abort_result: Result<(), JournalErrorV1> },
}

pub open spec fn reader_step_relation_v1(before: ReadContentsV1, after: ReadContentsV1, step: ReaderStepV1) -> bool {
    match step {
        ReaderStepV1::Acquire { consumer, requests, output_before, output_after, result } => {
            &&& requests.len() <= usize::MAX
            &&& requests.len() <= u64::MAX
            &&& acquire_execution_relation_v1(before, after, consumer, requests, output_before, output_after, result)
        },
        ReaderStepV1::Release { consumer, references, evidence_consumer, observed_free_capacity, result } => {
            &&& references.len() <= usize::MAX
            &&& release_execution_relation_v1(before, after, consumer, references, evidence_consumer, observed_free_capacity, result)
        },
        ReaderStepV1::Register { key, register_result } => {
            &&& reader_storage_frame_v1(before, after)
            &&& issuance_contents_frame_v1(before.journal, after.journal)
            &&& register_execution_relation_v1(before.journal.context_generation, before.journal.writer_capacity, key,
                before.journal.writers@, before.journal.free@, before.journal.registration_watermark, before.journal.reserved_count,
                after.journal.writers@, after.journal.free@, after.journal.registration_watermark, after.journal.reserved_count, register_result)
        },
        ReaderStepV1::Abort { reference, observed_free_capacity, abort_result } => {
            &&& reader_storage_frame_v1(before, after)
            &&& issuance_contents_frame_v1(before.journal, after.journal)
            &&& abort_execution_relation_v1(before.journal.context_generation, before.journal.writer_capacity,
                observed_free_capacity, reference, before.journal.writers@, before.journal.free@,
                before.journal.registration_watermark, before.journal.reserved_count, after.journal.writers@,
                after.journal.free@, after.journal.registration_watermark, after.journal.reserved_count, abort_result)
        },
    }
}

pub open spec fn reader_step_minted_v1(step: ReaderStepV1) -> Seq<ReadReferenceV1> {
    match step {
        ReaderStepV1::Acquire { output_after, result, .. } =>
            if result.is_ok() { Seq::new(output_after.len(), |i: int| output_after[i].unwrap()) } else { Seq::empty() },
        _ => Seq::empty(),
    }
}

pub proof fn reader_step_preserves_v1(before: ReadContentsV1, after: ReadContentsV1, step: ReaderStepV1)
    requires reader_invariant_v1(before), reader_step_relation_v1(before, after, step),
    ensures reader_invariant_v1(after), reader_epoch_step_v1(before.next_incarnation, after.next_incarnation, reader_step_minted_v1(step)),
{
    match step {
        ReaderStepV1::Acquire { consumer, requests, output_before, output_after, result } => {
            acquire_preserves_reader_invariant_v1(before, after, consumer, requests, output_before, output_after, result);
            acquire_epoch_projection_v1(before, after, consumer, requests, output_before, output_after, result);
        },
        ReaderStepV1::Release { consumer, references, evidence_consumer, observed_free_capacity, result } => {
            release_preserves_reader_invariant_v1(before, after, consumer, references, evidence_consumer, observed_free_capacity, result);
        },
        _ => { reader_allocation_frame_preserves_v1(before, after); },
    }
}

pub open spec fn reader_trace_relation_v1(states: Seq<ReadContentsV1>, steps: Seq<ReaderStepV1>) -> bool {
    &&& states.len() == steps.len() + 1
    &&& reader_invariant_v1(states[0])
    &&& states[0].next_incarnation == 1
    &&& forall|i: int| 0 <= i < steps.len() ==> reader_step_relation_v1(states[i], states[i + 1], #[trigger] steps[i])
}

pub proof fn reader_trace_prefix_preserves_v1(states: Seq<ReadContentsV1>, steps: Seq<ReaderStepV1>, end: nat)
    requires reader_trace_relation_v1(states, steps), end <= steps.len(),
    ensures reader_invariant_v1(states[end as int]),
    decreases end,
{
    if end > 0 {
        reader_trace_prefix_preserves_v1(states, steps, (end - 1) as nat);
        reader_step_preserves_v1(states[end - 1], states[end as int], steps[end - 1]);
    }
}

pub proof fn reader_trace_projects_epochs_v1(states: Seq<ReadContentsV1>, steps: Seq<ReaderStepV1>)
    requires reader_trace_relation_v1(states, steps),
    ensures reader_epoch_trace_v1(Seq::new(states.len(), |i: int| states[i].next_incarnation),
        Seq::new(steps.len(), |i: int| reader_step_minted_v1(steps[i]))),
{
    assert forall|i: int| 0 <= i < steps.len() implies
        reader_epoch_step_v1(states[i].next_incarnation, states[i + 1].next_incarnation, #[trigger] reader_step_minted_v1(steps[i])) by {
        reader_trace_prefix_preserves_v1(states, steps, i as nat);
        reader_step_preserves_v1(states[i], states[i + 1], steps[i]);
    }
}

pub proof fn reader_trace_never_reissues_v1(states: Seq<ReadContentsV1>, steps: Seq<ReaderStepV1>,
    first: nat, first_index: nat, last: nat, last_index: nat,
)
    requires reader_trace_relation_v1(states, steps), first <= last < steps.len(),
        first_index < reader_step_minted_v1(steps[first as int]).len(), last_index < reader_step_minted_v1(steps[last as int]).len(),
        first == last ==> first_index < last_index,
    ensures 0 < reader_step_minted_v1(steps[first as int])[first_index as int].incarnation
        < reader_step_minted_v1(steps[last as int])[last_index as int].incarnation,
{
    reader_trace_projects_epochs_v1(states, steps);
    let frontiers = Seq::new(states.len(), |i: int| states[i].next_incarnation);
    let minted = Seq::new(steps.len(), |i: int| reader_step_minted_v1(steps[i]));
    if first < last { reader_history_never_reissues_v1(frontiers, minted, first, first_index, last, last_index); }
    else { assert(reader_epoch_step_v1(frontiers[first as int], frontiers[(first + 1) as int], minted[first as int])); }
}

// Fixture enrollment is explicit and is not a production enrollment refinement.
pub fn reader_nonempty_witness_v1() -> (result: Option<ReadContentsV1>)
    ensures result.is_some(), reader_invariant_v1(result.unwrap()),
        result.unwrap().next_incarnation == 4,
        result.unwrap().readers@ == seq![1usize],
        result.unwrap().free_reads@ == seq![1usize],
        result.unwrap().leases@[0].is_some(), result.unwrap().leases@[0].unwrap().reference.incarnation == 3,
        result.unwrap().leases@[1].is_none(),
{
    let mut contents = match reader_constructor_verified_v1(7, 1, 1, 2) { Ok(value) => value, Err(_) => return None };
    let ghost empty = contents;
    let key = AllocationKeyV1 { context_generation: 7, local: 1 };
    let device = DeviceKeyV1 { context_generation: 7, local: 2 };
    let allocation = AllocationReferenceV1 { slot: 0, key };
    let _slot = contents.journal.allocation_free.pop();
    assert(_slot == Some(0));
    assert(contents.journal.allocation_free@ =~= Seq::<usize>::empty());
    contents.journal.allocations.set(0, Some(AllocationEntryV1 {
        key, device, byte_extent: 16, attempt_epoch: 0, content_lineage: 0, pending_member: None,
    }));
    proof { reader_allocation_frame_preserves_v1(empty, contents); }
    let first = WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission };
    let second = WriterKeyV1 { context_generation: 7, local: 12, kind: WriterKindV1::Submission };
    let request = AllocationReadV1 { allocation, device, byte_extent: 16, byte_offset: 0,
        byte_len: 8, attempt_epoch: 0, content_lineage: 0 };
    let mut requests = Vec::new();
    requests.push(request);
    proof {
        assert(requests@ =~= Seq::empty().push(request));
        read_slot_count_push_v1(Seq::empty(), request, 0);
        assert(read_slot_count_v1(requests@, 0) == 1);
    }
    let mut output_a = Vec::new();
    output_a.push(None);
    proof {
        assert(contents.free_reads@ =~= seq![1usize, 0usize]);
        assert(contents.leases@ =~= seq![None, None]);
        assert(contents.readers@ =~= seq![0usize]);
        assert(read_decision_v1(contents.journal, request) == Ok(()));
        reveal_with_fuel(acquire_scan_v1, 2);
        assert(acquire_decision_v1(contents, first, requests@, output_a@) == Ok(()));
    }
    if acquire_verified_v1(&mut contents, first, requests.as_slice(), &mut output_a).is_err() { return None; }
    let reference_a = output_a[0].unwrap();
    assert(reference_a.slot == 0 && reference_a.incarnation == 1);
    let mut output_b = Vec::new();
    output_b.push(None);
    proof {
        reveal_with_fuel(acquire_scan_v1, 2);
        assert(contents.readers@[0] == 1);
        assert(contents.free_reads@ =~= seq![1usize]);
        assert(acquire_decision_v1(contents, second, requests@, output_b@) == Ok(()));
    }
    if acquire_verified_v1(&mut contents, second, requests.as_slice(), &mut output_b).is_err() { return None; }
    let reference_b = output_b[0].unwrap();
    assert(reference_b.slot == 1 && reference_b.incarnation == 2);
    assert(contents.readers@[0] == 2);
    let mut release = Vec::new();
    release.push(reference_b);
    proof {
        reveal_with_fuel(release_scan_v1, 2);
        assert(released_requests_v1(contents, release@) =~= requests@);
        assert(released_requests_v1(contents, release@).take(1) =~= requests@);
        assert(release_decision_v1(contents, second, release@, second, 2) == Ok(()));
    }
    if release_verified_v1(&mut contents, second, release.as_slice(), second, 2).is_err() { return None; }
    assert(contents.readers@[0] == 1);
    proof {
        reveal_with_fuel(acquired_leases_v1, 3);
        reveal_with_fuel(released_leases_v1, 3);
        assert(contents.leases@[0] == Some(ReadLeaseV1 { reference: reference_a, request }));
    }
    let mut allocations = Vec::new();
    allocations.push(allocation);
    let busy = require_unread_exec_v1(&contents, allocations.as_slice());
    assert(busy == Err(ReadErrorV1::AllocationBusy));
    release.set(0, reference_a);
    proof {
        reveal_with_fuel(release_scan_v1, 2);
        assert(released_requests_v1(contents, release@) =~= requests@);
        assert(released_requests_v1(contents, release@).take(1) =~= requests@);
        assert(release_decision_v1(contents, first, release@, first, 2) == Ok(()));
    }
    if release_verified_v1(&mut contents, first, release.as_slice(), first, 2).is_err() { return None; }
    assert(contents.readers@[0] == 0);
    assert(contents.free_reads@ == seq![1usize, 0usize]);
    output_a.set(0, None);
    proof {
        reveal_with_fuel(acquire_scan_v1, 2);
        assert(acquire_decision_v1(contents, first, requests@, output_a@) == Ok(()));
    }
    if acquire_verified_v1(&mut contents, first, requests.as_slice(), &mut output_a).is_err() { return None; }
    assert(output_a@[0].unwrap().slot == reference_a.slot);
    let stale = lease_lookup_exec_v1(&contents, reference_a);
    assert(stale == Err(ReadErrorV1::InvalidReference));
    Some(contents)
}

}

verus! {
pub open spec fn reader_mutation_prestate_v1(contents: ReadContentsV1) -> bool { reader_invariant_v1(contents) }
pub fn reader_invariant_subject_v1(contents: &mut ReadContentsV1)
    requires reader_mutation_prestate_v1(*old(contents)),
    ensures reader_invariant_v1(*final(contents)),
{
    if let Some(mut entry) = contents.leases[0] { entry.reference.consumer.local = 0; contents.leases.set(0, Some(entry)); }
    return ();
}
}

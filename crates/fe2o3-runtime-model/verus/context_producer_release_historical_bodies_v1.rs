// Exact release declaration blocks from the historical lifecycle, in the importing type universe.
use super::*;

verus! {

pub proof fn producer_arena_remove_v1(journal: JournalContentsV1,
    entries: Seq<Option<ProducerReservationV1>>, free: Seq<usize>, counts: Seq<usize>, next: u64, slot: usize)
    requires producer_arena_v1(journal, entries, free, counts, next), counts.len() <= usize::MAX,
        slot < entries.len(), entries[slot as int].is_some(),
        entries[slot as int].unwrap().request.read.allocation.slot < counts.len(),
        free.len() < entries.len(), counts[entries[slot as int].unwrap().request.read.allocation.slot as int] > 0,
    ensures producer_arena_v1(journal, entries.update(slot as int, None), free.push(slot),
        counts.update(entries[slot as int].unwrap().request.read.allocation.slot as int,
            (counts[entries[slot as int].unwrap().request.read.allocation.slot as int] - 1) as usize), next),
{
    let entry = entries[slot as int].unwrap();
    assert(producer_entry_valid_v1(journal, entry, slot as int, next));
    reveal(producer_entry_valid_v1);
    let after = entries.update(slot as int, None);
    assert(!free.contains(slot));
    assert forall|s: int| 0 <= s < after.len() implies (#[trigger] after[s]).is_none() == free.push(slot).contains(s as usize) by {
        if s == slot { assert(free.push(slot)[free.len() as int] == s); }
        else if entries[s].is_none() {
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
    assert forall|a: int| 0 <= a < counts.len() implies
        counts.update(entry.request.read.allocation.slot as int, (counts[entry.request.read.allocation.slot as int] - 1) as usize)[a]
            == #[trigger] producer_count_v1(after, a as usize) by {
        producer_count_update_v1(entries, slot as int, None, a as usize);
        assert(counts[a] == producer_count_v1(entries, a as usize));
    }
}

// Single-slot logical transitions only; not canonical batch preflight/commit refinement.
pub open spec fn producer_release_header_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    evidence_consumer: WriterKeyV1, count: usize, observed_free_capacity: usize) -> Result<(), ReadErrorV1>
{
    if !same_key_v1(evidence_consumer, consumer) { Err(ReadErrorV1::SettlementEvidenceMismatch) }
    else if count == 0 { Err(ReadErrorV1::RosterCapacity) }
    else if contents.free@.len() + count > usize::MAX || contents.free@.len() + count > contents.reservations@.len()
        || contents.free@.len() + count > observed_free_capacity { Err(ReadErrorV1::InvalidState) }
    else { Ok(()) }
}

#[verifier::spinoff_prover]
pub fn producer_release_header_exec_v1(contents: &ProducerReadContentsV1, consumer: WriterKeyV1,
    evidence_consumer: WriterKeyV1, count: usize, observed_free_capacity: usize) -> (result: Result<(), ReadErrorV1>)
    ensures exact_decision_v1(result, producer_release_header_v1(*contents, consumer, evidence_consumer, count, observed_free_capacity)),
{
    if !same_key_exec_v1(evidence_consumer, consumer) { return Err(ReadErrorV1::SettlementEvidenceMismatch); }
    if count == 0 { return Err(ReadErrorV1::RosterCapacity); }
    let count = match contents.free.len().checked_add(count) {
        Some(value) => value, None => return Err(ReadErrorV1::InvalidState),
    };
    if count > contents.reservations.len() || count > observed_free_capacity { return Err(ReadErrorV1::InvalidState); }
    Ok(())
}

pub open spec fn producer_release_item_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    reference: ProducerReadReferenceV1, state: ReadScanV1) -> Result<ReadScanV1, ReadErrorV1>
{
    if !same_key_v1(reference.consumer, consumer) { Err(ReadErrorV1::InvalidReference) }
    else { match producer_lookup_decision_v1(contents, reference) {
        Err(error) => Err(error),
        Ok(request) => {
            let key = read_order_v1(request.read, reference.incarnation);
            if state.previous.is_some() && !order_lt_v1(state.previous.unwrap(), key) { Err(ReadErrorV1::NonCanonicalRoster) }
            else {
                let group = next_group_v1(state.previous, state.group, key);
                if contents.counts@[request.read.allocation.slot as int] < group { Err(ReadErrorV1::InvalidState) }
                else { Ok(ReadScanV1 { previous: Some(key), group }) }
            }
        },
    } }
}

pub fn producer_release_item_exec_v1(contents: &ProducerReadContentsV1, consumer: WriterKeyV1,
    reference: ProducerReadReferenceV1, state: ReadScanV1) -> (result: Result<ReadScanV1, ReadErrorV1>)
    requires contents.counts@.len() == contents.stable.journal.allocations@.len(), state.group < usize::MAX,
    ensures exact_decision_v1(result, producer_release_item_v1(*contents, consumer, reference, state)),
        match result { Ok(next) => next.group <= state.group + 1, Err(_) => true },
{
    if !same_key_exec_v1(reference.consumer, consumer) { return Err(ReadErrorV1::InvalidReference); }
    let request = match producer_lookup_exec_v1(contents, reference) {
        Err(error) => return Err(error), Ok(value) => value,
    };
    let key = (request.read.allocation.key.local, request.read.byte_offset, request.read.byte_len, reference.incarnation);
    if let Some(prior) = state.previous {
        if !order_lt_exec_v1(prior, key) { return Err(ReadErrorV1::NonCanonicalRoster); }
    }
    let group = next_group_exec_v1(state.previous, state.group, key);
    if contents.counts[request.read.allocation.slot] < group { return Err(ReadErrorV1::InvalidState); }
    Ok(ReadScanV1 { previous: Some(key), group })
}

pub open spec fn producer_release_scan_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    references: Seq<ProducerReadReferenceV1>, index: nat, state: ReadScanV1) -> Result<(), ReadErrorV1>
    decreases references.len() - index,
{
    if index >= references.len() { Ok(()) }
    else { match producer_release_item_v1(contents, consumer, references[index as int], state) {
        Err(error) => Err(error),
        Ok(next) => producer_release_scan_v1(contents, consumer, references, index + 1, next),
    } }
}

pub open spec fn producer_release_decision_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    references: Seq<ProducerReadReferenceV1>, evidence_consumer: WriterKeyV1, observed_free_capacity: usize) -> Result<(), ReadErrorV1>
{
    match producer_release_header_v1(contents, consumer, evidence_consumer, references.len() as usize, observed_free_capacity) {
        Err(error) => Err(error),
        Ok(()) => producer_release_scan_v1(contents, consumer, references, 0, ReadScanV1 { previous: None, group: 0 }),
    }
}

#[verifier::spinoff_prover]
pub fn producer_release_preflight_exec_v1(contents: &ProducerReadContentsV1, consumer: WriterKeyV1,
    references: &[ProducerReadReferenceV1], evidence_consumer: WriterKeyV1, observed_free_capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires contents.counts@.len() == contents.stable.journal.allocations@.len(),
    ensures exact_decision_v1(result, producer_release_decision_v1(*contents, consumer, references@, evidence_consumer, observed_free_capacity)),
{
    match producer_release_header_exec_v1(contents, consumer, evidence_consumer, references.len(), observed_free_capacity) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    let mut index = 0;
    let mut state = ReadScanV1 { previous: None, group: 0 };
    while index < references.len()
        invariant index <= references.len(), state.group <= index,
            contents.counts@.len() == contents.stable.journal.allocations@.len(),
            producer_release_decision_v1(*contents, consumer, references@, evidence_consumer, observed_free_capacity)
                == producer_release_scan_v1(*contents, consumer, references@, index as nat, state),
        decreases references.len() - index,
    {
        match producer_release_item_exec_v1(contents, consumer, references[index], state) {
            Err(error) => return Err(error), Ok(next) => state = next,
        }
        index += 1;
    }
    Ok(())
}

pub open spec fn producer_released_requests_v1(before: ProducerReadContentsV1, references: Seq<ProducerReadReferenceV1>) -> Seq<ProducerReadV1> {
    Seq::new(references.len(), |i: int| before.reservations@[references[i].slot as int].unwrap().request)
}

pub open spec fn producer_release_commit_ready_v1(before: ProducerReadContentsV1, references: Seq<ProducerReadReferenceV1>) -> bool {
    &&& before.free@.len() + references.len() <= usize::MAX
    &&& before.free@.len() + references.len() <= before.reservations@.len()
    &&& forall|i: int, j: int| 0 <= i < j < references.len() ==> references[i].slot != references[j].slot
    &&& producer_release_prefix_ready_v1(before, references, references.len())
}

pub open spec fn producer_release_prefix_ready_v1(before: ProducerReadContentsV1, references: Seq<ProducerReadReferenceV1>, end: nat) -> bool {
    forall|i: int| 0 <= i < end ==> {
        let slot = (#[trigger] references[i]).slot;
        let entry = before.reservations@[slot as int].unwrap();
        &&& slot < before.reservations@.len()
        &&& before.reservations@[slot as int].is_some()
        &&& same_producer_read_reference_v1(entry.reference, references[i])
        &&& entry.request.read.allocation.slot < before.counts@.len()
        &&& producer_request_count_v1(producer_released_requests_v1(before, references).take(i + 1), entry.request.read.allocation.slot)
            <= before.counts@[entry.request.read.allocation.slot as int]
    }
}

pub open spec fn producer_released_reservations_v1(before: ProducerReadContentsV1, references: Seq<ProducerReadReferenceV1>, count: nat)
    -> Seq<Option<ProducerReservationV1>>
    decreases count,
{
    if count == 0 { before.reservations@ }
    else { producer_released_reservations_v1(before, references, (count - 1) as nat).update(references[count - 1].slot as int, None) }
}

pub proof fn producer_released_reservations_unselected_v1(before: ProducerReadContentsV1, references: Seq<ProducerReadReferenceV1>, count: nat, slot: int)
    requires count <= references.len(), 0 <= slot < before.reservations@.len(),
        forall|i: int| 0 <= i < count ==> references[i].slot != slot,
        forall|i: int| 0 <= i < count ==> references[i].slot < before.reservations@.len(),
    ensures producer_released_reservations_v1(before, references, count)[slot] == before.reservations@[slot],
        producer_released_reservations_v1(before, references, count).len() == before.reservations@.len(),
    decreases count,
{
    if count > 0 { producer_released_reservations_unselected_v1(before, references, (count - 1) as nat, slot); }
}

pub open spec fn producer_release_commit_prefix_v1(
    before: ProducerReadContentsV1, after: ProducerReadContentsV1, references: Seq<ProducerReadReferenceV1>, count: nat,
) -> bool {
    &&& after.stable == before.stable
    &&& after.next_incarnation == before.next_incarnation
    &&& after.free@ == before.free@ + Seq::new(count, |i: int| references[i].slot)
    &&& after.reservations@ == producer_released_reservations_v1(before, references, count)
    &&& after.reservations@.len() == before.reservations@.len()
    &&& after.counts@.len() == before.counts@.len()
    &&& forall|a: int| 0 <= a < after.counts@.len() ==>
        after.counts@[a] == before.counts@[a]
            - producer_request_count_v1(producer_released_requests_v1(before, references).take(count as int), a as usize)
}

pub fn producer_release_commit_exec_v1(contents: &mut ProducerReadContentsV1, references: &[ProducerReadReferenceV1])
    requires producer_release_commit_ready_v1(*old(contents), references@),
    ensures producer_release_commit_prefix_v1(*old(contents), *final(contents), references@, references@.len()),
{
    let ghost before = *contents;
    let ghost requests = producer_released_requests_v1(before, references@);
    let allocation_capacity = contents.counts.len();
    let mut index = 0;
    proof {
        assert(Seq::new(0, |i: int| references@[i].slot) =~= Seq::<usize>::empty());
        assert(before.free@ + Seq::<usize>::empty() =~= before.free@);
        assert(requests.take(0) =~= Seq::<ProducerReadV1>::empty());
    }
    while index < references.len()
        invariant index <= references.len(), producer_release_commit_ready_v1(before, references@),
            before.counts@.len() == allocation_capacity,
            requests == producer_released_requests_v1(before, references@),
            producer_release_commit_prefix_v1(before, *contents, references@, index as nat),
        decreases references.len() - index,
    {
        let reference = references[index];
        let ghost previous_counts = contents.counts@;
        proof { producer_released_reservations_unselected_v1(before, references@, index as nat, reference.slot as int); }
        let entry = contents.reservations[reference.slot].unwrap();
        contents.reservations.set(reference.slot, None);
        let allocation = entry.request.read.allocation.slot;
        proof {
            assert(requests.take(index + 1) =~= requests.take(index as int).push(requests[index as int]));
            producer_request_count_push_v1(requests.take(index as int), requests[index as int], allocation);
        }
        let count = contents.counts[allocation] - 1;
        contents.counts.set(allocation, count);
        contents.free.push(reference.slot);
        proof {
            assert forall|a: int| 0 <= a < contents.counts@.len() implies
                contents.counts@[a] == before.counts@[a] - producer_request_count_v1(requests.take(index + 1), a as usize) by {
                assert(previous_counts[a] == before.counts@[a] - producer_request_count_v1(requests.take(index as int), a as usize));
                assert(requests[index as int].read.allocation.slot == allocation);
                assert(contents.counts@[a] == if a == allocation { previous_counts[a] - 1 } else { previous_counts[a] as int });
                producer_request_count_push_v1(requests.take(index as int), requests[index as int], a as usize);
            }
            assert(contents.free@ =~= before.free@ + Seq::new((index + 1) as nat, |i: int| references@[i].slot));
        }
        index += 1;
    }
}

#[verifier::spinoff_prover]
pub proof fn producer_release_scan_ready_v1(before: ProducerReadContentsV1, consumer: WriterKeyV1,
    references: Seq<ProducerReadReferenceV1>, evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
    index: nat, state: ReadScanV1)
    requires before.counts@.len() == before.stable.journal.allocations@.len(), references.len() <= usize::MAX,
        producer_release_header_v1(before, consumer, evidence_consumer, references.len() as usize, observed_free_capacity) == Ok(()),
        index <= references.len(), producer_release_scan_v1(before, consumer, references, index, state) == Ok(()),
        canonical_prefix_v1(Seq::new(references.len(), |i: int|
            read_order_v1(producer_released_requests_v1(before, references)[i].read, references[i].incarnation)), index, state),
        forall|i: int| 0 <= i < index ==>
            producer_status_decision_v1(before.stable.journal, #[trigger] producer_released_requests_v1(before, references)[i]).is_ok(),
        producer_release_prefix_ready_v1(before, references, index),
    ensures producer_release_commit_ready_v1(before, references),
    decreases references.len() - index,
{
    let requests = producer_released_requests_v1(before, references);
    let keys = Seq::new(references.len(), |i: int| read_order_v1(requests[i].read, references[i].incarnation));
    if index < references.len() {
        let reference = references[index as int];
        let next = match producer_release_item_v1(before, consumer, reference, state) { Ok(next) => next, Err(_) => state };
        canonical_prefix_extend_v1(keys, index, state);
        producer_count_local_dominance_v1(before.stable.journal, requests.take((index + 1) as int), keys.take((index + 1) as int), requests[index as int]);
        assert(next == ReadScanV1 { previous: Some(keys[index as int]),
            group: next_group_v1(state.previous, state.group, keys[index as int]) });
        producer_release_scan_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity, index + 1, next);
    } else {
        assert forall|i: int, j: int| 0 <= i < j < references.len() implies references[i].slot != references[j].slot by {
            if references[i].slot == references[j].slot {
                assert(keys[i] == keys[j]);
                assert(order_lt_v1(keys[i], keys[j]));
            }
        }
    }
}

pub proof fn producer_release_preflight_ready_v1(before: ProducerReadContentsV1, consumer: WriterKeyV1,
    references: Seq<ProducerReadReferenceV1>, evidence_consumer: WriterKeyV1, observed_free_capacity: usize)
    requires before.counts@.len() == before.stable.journal.allocations@.len(), references.len() <= usize::MAX,
        producer_release_decision_v1(before, consumer, references, evidence_consumer, observed_free_capacity) == Ok(()),
    ensures producer_release_commit_ready_v1(before, references),
        before.free@.len() + references.len() <= observed_free_capacity,
{
    producer_release_scan_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity,
        0, ReadScanV1 { previous: None, group: 0 });
}

pub open spec fn producer_released_counts_v1(before: ProducerReadContentsV1, references: Seq<ProducerReadReferenceV1>, count: nat) -> Seq<usize> {
    Seq::new(before.counts@.len(), |a: int|
        (before.counts@[a] - producer_request_count_v1(producer_released_requests_v1(before, references).take(count as int), a as usize)) as usize)
}

#[verifier::spinoff_prover]
pub proof fn producer_release_arena_prefix_v1(before: ProducerReadContentsV1, references: Seq<ProducerReadReferenceV1>, count: nat)
    requires producer_invariant_v1(before), producer_release_commit_ready_v1(before, references), count <= references.len(),
    ensures producer_arena_v1(before.stable.journal, producer_released_reservations_v1(before, references, count),
        before.free@ + Seq::new(count, |i: int| references[i].slot), producer_released_counts_v1(before, references, count),
        before.next_incarnation),
    decreases count,
{
    if count == 0 {
        assert(producer_released_requests_v1(before, references).take(0) =~= Seq::<ProducerReadV1>::empty());
        assert(producer_released_counts_v1(before, references, 0) =~= before.counts@);
        assert(before.free@ + Seq::new(0, |i: int| references[i].slot) =~= before.free@);
    } else {
        let n = (count - 1) as nat;
        producer_release_arena_prefix_v1(before, references, n);
        let slot = references[n as int].slot;
        producer_released_reservations_unselected_v1(before, references, n, slot as int);
        let entry = before.reservations@[slot as int].unwrap();
        let reservations = producer_released_reservations_v1(before, references, n);
        let free = before.free@ + Seq::new(n, |i: int| references[i].slot);
        let counts = producer_released_counts_v1(before, references, n);
        let requests = producer_released_requests_v1(before, references);
        assert(requests.take(count as int) =~= requests.take(n as int).push(requests[n as int]));
        producer_request_count_push_v1(requests.take(n as int), entry.request, entry.request.read.allocation.slot);
        producer_arena_remove_v1(before.stable.journal, reservations, free, counts, before.next_incarnation, slot);
        assert(free.push(slot) =~= before.free@ + Seq::new(count, |i: int| references[i].slot));
        assert(producer_released_counts_v1(before, references, count) =~=
            counts.update(entry.request.read.allocation.slot as int, (counts[entry.request.read.allocation.slot as int] - 1) as usize)) by {
            assert forall|a: int| 0 <= a < counts.len() implies #[trigger] producer_released_counts_v1(before, references, count)[a]
                == counts.update(entry.request.read.allocation.slot as int, (counts[entry.request.read.allocation.slot as int] - 1) as usize)[a] by {
                producer_request_count_push_v1(requests.take(n as int), entry.request, a as usize);
            }
        }
    }
}

pub open spec fn producer_release_execution_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, references: Seq<ProducerReadReferenceV1>, evidence_consumer: WriterKeyV1,
    observed_free_capacity: usize, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == producer_release_decision_v1(before, consumer, references, evidence_consumer, observed_free_capacity)
    &&& match result {
        Err(_) => producer_contents_frame_v1(before, after),
        Ok(()) => producer_release_commit_prefix_v1(before, after, references, references.len()),
    }
}

pub proof fn producer_release_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, references: Seq<ProducerReadReferenceV1>, evidence_consumer: WriterKeyV1,
    observed_free_capacity: usize, result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(before), references.len() <= usize::MAX,
        producer_release_execution_relation_v1(before, after, consumer, references, evidence_consumer, observed_free_capacity, result),
    ensures producer_invariant_v1(after), after.next_incarnation == before.next_incarnation,
        producer_live_reads_v1(after) == producer_live_reads_v1(before) - if result.is_ok() { references.len() as int } else { 0 },
{
    if result.is_ok() {
        if let Ok(value) = result { assert(value =~= ()); }
        producer_release_preflight_ready_v1(before, consumer, references, evidence_consumer, observed_free_capacity);
        producer_release_arena_prefix_v1(before, references, references.len());
        assert(producer_released_requests_v1(before, references).take(references.len() as int)
            =~= producer_released_requests_v1(before, references));
        assert(after.counts@ =~= producer_released_counts_v1(before, references, references.len()));
    }
}

#[verifier::spinoff_prover]
pub fn producer_release_contents_exec_v1(contents: &mut ProducerReadContentsV1, consumer: WriterKeyV1,
    references: &[ProducerReadReferenceV1], evidence_consumer: WriterKeyV1, observed_free_capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*old(contents)),
    ensures producer_release_execution_relation_v1(*old(contents), *final(contents), consumer, references@, evidence_consumer, observed_free_capacity, result),
{
    let ghost before = *contents;
    let _count = references.len();
    let decision = producer_release_preflight_exec_v1(contents, consumer, references, evidence_consumer, observed_free_capacity);
    proof { assert(decision == producer_release_decision_v1(before, consumer, references@, evidence_consumer, observed_free_capacity)); }
    match decision {
        Err(error) => return Err(error), Ok(value) => { proof { assert(value =~= ()); } },
    }
    proof { producer_release_preflight_ready_v1(before, consumer, references@, evidence_consumer, observed_free_capacity); }
    producer_release_commit_exec_v1(contents, references);
    Ok(())
}

pub fn producer_release_exec_v1(contents: &mut ProducerReadContentsV1, consumer: WriterKeyV1,
    references: &[ProducerReadReferenceV1], evidence_consumer: WriterKeyV1, observed_free_capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*old(contents)),
    ensures producer_release_execution_relation_v1(*old(contents), *final(contents), consumer, references@, evidence_consumer, observed_free_capacity, result),
        producer_invariant_v1(*final(contents)), final(contents).next_incarnation == old(contents).next_incarnation,
        producer_live_reads_v1(*final(contents)) == producer_live_reads_v1(*old(contents)) - if result.is_ok() { references@.len() as int } else { 0 },
        result.is_ok() ==> final(contents).free@.len() <= observed_free_capacity,
{
    let ghost before = *contents;
    let _count = references.len();
    let result = producer_release_contents_exec_v1(contents, consumer, references, evidence_consumer, observed_free_capacity);
    proof { producer_release_preserves_v1(before, *contents, consumer, references@, evidence_consumer, observed_free_capacity, result); }
    result
}

}

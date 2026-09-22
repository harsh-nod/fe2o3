// Exact declaration blocks from the historical lifecycle, sharing the importing type universe.
use super::*;

verus! {

pub open spec fn producer_live_reads_v1(contents: ProducerReadContentsV1) -> int {
    contents.stable.leases@.len() - contents.stable.free_reads@.len()
        + contents.reservations@.len() - contents.free@.len()
}

pub proof fn producer_capacity_arithmetic_v1(contents: ProducerReadContentsV1)
    requires producer_invariant_v1(contents),
    ensures 0 <= producer_live_reads_v1(contents) <= contents.reservations@.len() <= usize::MAX,
        0 <= contents.reservations@.len() - producer_live_reads_v1(contents) <= usize::MAX,
{}

pub proof fn producer_count_update_v1(entries: Seq<Option<ProducerReservationV1>>, slot: int,
    entry: Option<ProducerReservationV1>, allocation: usize)
    requires 0 <= slot < entries.len(),
    ensures producer_count_v1(entries.update(slot, entry), allocation) + producer_weight_v1(entries[slot], allocation)
        == producer_count_v1(entries, allocation) + producer_weight_v1(entry, allocation),
    decreases entries.len(),
{
    if slot < entries.len() - 1 {
        assert(entries.update(slot, entry).drop_last() =~= entries.drop_last().update(slot, entry));
        producer_count_update_v1(entries.drop_last(), slot, entry, allocation);
    } else {
        assert(entries.update(slot, entry).drop_last() =~= entries.drop_last());
    }
}

pub proof fn producer_arena_insert_v1(journal: JournalContentsV1,
    entries: Seq<Option<ProducerReservationV1>>, free: Seq<usize>, counts: Seq<usize>, next: u64,
    entry: ProducerReservationV1)
    requires producer_arena_v1(journal, entries, free, counts, next), free.len() > 0, counts.len() <= usize::MAX,
        entry.reference.slot == free.last(), entry.reference.incarnation == next, next < u64::MAX,
        producer_entry_valid_v1(journal, entry, entry.reference.slot as int, (next + 1) as u64),
        producer_status_v1(journal, entry.request) == Some(ProducerStatusV1::Pending),
        entry.request.read.allocation.slot < counts.len(),
        counts[entry.request.read.allocation.slot as int] < usize::MAX,
    ensures producer_arena_v1(journal, entries.update(entry.reference.slot as int, Some(entry)), free.drop_last(),
        counts.update(entry.request.read.allocation.slot as int, (counts[entry.request.read.allocation.slot as int] + 1) as usize),
        (next + 1) as u64),
{
    let slot = entry.reference.slot as int;
    let after = entries.update(slot, Some(entry));
    assert(free.contains(slot as usize));
    assert(entries[slot].is_none());
    assert(!free.drop_last().contains(slot as usize)) by {
        if free.drop_last().contains(slot as usize) {
            let i = choose|i: int| 0 <= i < free.drop_last().len() && free.drop_last()[i] == slot;
            assert(free[i] == free[free.len() - 1]);
        }
    }
    assert forall|s: int| 0 <= s < after.len() implies (#[trigger] after[s]).is_none() == free.drop_last().contains(s as usize) by {
        if s != slot && free.contains(s as usize) {
            let i = choose|i: int| 0 <= i < free.len() && free[i] == s;
            assert(i < free.len() - 1);
            assert(free.drop_last()[i] == s);
        }
    }
    assert forall|a: int| 0 <= a < counts.len() implies
        counts.update(entry.request.read.allocation.slot as int, (counts[entry.request.read.allocation.slot as int] + 1) as usize)[a]
            == #[trigger] producer_count_v1(after, a as usize) by {
        producer_count_update_v1(entries, slot, Some(entry), a as usize);
        assert(counts[a] == producer_count_v1(entries, a as usize));
    }
    assert forall|s: int| 0 <= s < after.len() && (#[trigger] after[s]).is_some() implies
        producer_entry_valid_v1(journal, after[s].unwrap(), s, (next + 1) as u64) by {
        if s != slot {
            assert(producer_entry_valid_v1(journal, entries[s].unwrap(), s, next));
            reveal(producer_entry_valid_v1);
        }
    }
    assert forall|s: int, t: int| 0 <= s < t < after.len()
        && (#[trigger] after[s]).is_some() && (#[trigger] after[t]).is_some()
        implies after[s].unwrap().reference.incarnation != after[t].unwrap().reference.incarnation by {
        if s != slot { assert(producer_entry_valid_v1(journal, entries[s].unwrap(), s, next)); }
        if t != slot { assert(producer_entry_valid_v1(journal, entries[t].unwrap(), t, next)); }
        reveal(producer_entry_valid_v1);
    }
}

pub open spec fn producer_outer_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1) -> bool {
    &&& after.reservations@ == before.reservations@
    &&& after.free@ == before.free@
    &&& after.counts@ == before.counts@
    &&& after.next_incarnation == before.next_incarnation
}

pub open spec fn producer_capacity_decision_v1(contents: ProducerReadContentsV1, count: usize) -> Result<(), ReadErrorV1> {
    if count > contents.reservations@.len() - producer_live_reads_v1(contents) { Err(ReadErrorV1::MemberCapacity) }
    else if contents.next_incarnation == 0 || contents.next_incarnation + count > u64::MAX { Err(ReadErrorV1::EpochExhausted) }
    else { Ok(()) }
}

#[verifier::spinoff_prover]
pub fn producer_capacity_exec_v1(contents: &ProducerReadContentsV1, count: usize) -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*contents), count <= u64::MAX,
    ensures exact_decision_v1(result, producer_capacity_decision_v1(*contents, count)),
{
    proof { producer_capacity_arithmetic_v1(*contents); }
    let stable_live = contents.stable.leases.len() - contents.stable.free_reads.len();
    let producer_live = contents.reservations.len() - contents.free.len();
    let remaining = contents.reservations.len() - (stable_live + producer_live);
    if count > remaining { return Err(ReadErrorV1::MemberCapacity); }
    if contents.next_incarnation == 0 || contents.next_incarnation.checked_add(count as u64).is_none() {
        return Err(ReadErrorV1::EpochExhausted);
    }
    Ok(())
}

pub fn producer_output_vacant_exec_v1(output: &[Option<ProducerReadReferenceV1>]) -> (result: bool)
    ensures result == (forall|i: int| 0 <= i < output@.len() ==> output@[i].is_none()),
{
    let mut index = 0;
    while index < output.len()
        invariant index <= output.len(), forall|i: int| 0 <= i < index ==> output@[i].is_none(),
        decreases output.len() - index,
    {
        if output[index].is_some() { return false; }
        index += 1;
    }
    true
}

pub open spec fn producer_acquire_header_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    count: usize, output: Seq<Option<ProducerReadReferenceV1>>) -> Result<(), ReadErrorV1>
{
    if consumer.context_generation != contents.stable.journal.context_generation { Err(ReadErrorV1::ForeignContext) }
    else if !issuable_id_v1(consumer.local) || consumer.kind != WriterKindV1::Submission { Err(ReadErrorV1::InvalidWriterId) }
    else if count == 0 || count != output.len() { Err(ReadErrorV1::RosterCapacity) }
    else if exists|i: int| 0 <= i < output.len() && output[i].is_some() { Err(ReadErrorV1::InvalidState) }
    else { producer_capacity_decision_v1(contents, count) }
}

#[verifier::spinoff_prover]
pub fn producer_acquire_header_exec_v1(contents: &ProducerReadContentsV1, consumer: WriterKeyV1,
    count: usize, output: &[Option<ProducerReadReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*contents), count <= u64::MAX,
    ensures exact_decision_v1(result, producer_acquire_header_v1(*contents, consumer, count, output@)),
{
    if consumer.context_generation != contents.stable.journal.context_generation { return Err(ReadErrorV1::ForeignContext); }
    if !issuable_id_exec_v1(consumer.local) || !matches!(consumer.kind, WriterKindV1::Submission) { return Err(ReadErrorV1::InvalidWriterId); }
    if count == 0 || count != output.len() { return Err(ReadErrorV1::RosterCapacity); }
    if !producer_output_vacant_exec_v1(output) { return Err(ReadErrorV1::InvalidState); }
    producer_capacity_exec_v1(contents, count)
}

pub open spec fn producer_acquire_item_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    request: ProducerReadV1, index: usize, state: ReadScanV1) -> Result<ReadScanV1, ReadErrorV1>
{
    match producer_validate_decision_v1(contents.stable.journal, request) {
        Err(error) => Err(error),
        Ok(()) => {
            if request.producer.key.local >= consumer.local { Err(ReadErrorV1::InvalidWriterId) }
            else {
                let key = read_order_v1(request.read, 0);
                if state.previous.is_some() && !order_lt_v1(state.previous.unwrap(), key) { Err(ReadErrorV1::NonCanonicalRoster) }
                else {
                    let group = next_group_v1(state.previous, state.group, key);
                    let count = contents.counts@[request.read.allocation.slot as int] + group;
                    if count > usize::MAX || count > contents.reservations@.len() { Err(ReadErrorV1::InvalidState) }
                    else {
                        let slot = contents.free@[contents.free@.len() - index - 1];
                        if slot >= contents.reservations@.len() || contents.reservations@[slot as int].is_some() {
                            Err(ReadErrorV1::InvalidState)
                        } else { Ok(ReadScanV1 { previous: Some(key), group }) }
                    }
                }
            }
        },
    }
}

#[verifier::spinoff_prover]
pub fn producer_acquire_item_exec_v1(contents: &ProducerReadContentsV1, consumer: WriterKeyV1,
    request: ProducerReadV1, index: usize, state: ReadScanV1) -> (result: Result<ReadScanV1, ReadErrorV1>)
    requires contents.counts@.len() == contents.stable.journal.allocations@.len(),
        index < contents.free@.len(), state.group < usize::MAX,
    ensures exact_decision_v1(result, producer_acquire_item_v1(*contents, consumer, request, index, state)),
        match result { Ok(next) => next.group <= state.group + 1, Err(_) => true },
{
    match producer_validate_exec_v1(&contents.stable.journal, request) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    if request.producer.key.local >= consumer.local { return Err(ReadErrorV1::InvalidWriterId); }
    let key = (request.read.allocation.key.local, request.read.byte_offset, request.read.byte_len, 0);
    if let Some(prior) = state.previous {
        if !order_lt_exec_v1(prior, key) { return Err(ReadErrorV1::NonCanonicalRoster); }
    }
    let group = next_group_exec_v1(state.previous, state.group, key);
    let count = match contents.counts[request.read.allocation.slot].checked_add(group) {
        Some(value) => value, None => return Err(ReadErrorV1::InvalidState),
    };
    if count > contents.reservations.len() { return Err(ReadErrorV1::InvalidState); }
    let slot = contents.free[contents.free.len() - index - 1];
    if slot >= contents.reservations.len() || contents.reservations[slot].is_some() { return Err(ReadErrorV1::InvalidState); }
    Ok(ReadScanV1 { previous: Some(key), group })
}

pub open spec fn producer_acquire_scan_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ProducerReadV1>, index: nat, state: ReadScanV1) -> Result<(), ReadErrorV1>
    decreases requests.len() - index,
{
    if index >= requests.len() { Ok(()) }
    else { match producer_acquire_item_v1(contents, consumer, requests[index as int], index as usize, state) {
        Err(error) => Err(error),
        Ok(next) => producer_acquire_scan_v1(contents, consumer, requests, index + 1, next),
    } }
}

pub open spec fn producer_acquire_decision_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ProducerReadV1>, output: Seq<Option<ProducerReadReferenceV1>>) -> Result<(), ReadErrorV1>
{
    match producer_acquire_header_v1(contents, consumer, requests.len() as usize, output) {
        Err(error) => Err(error),
        Ok(()) => producer_acquire_scan_v1(contents, consumer, requests, 0, ReadScanV1 { previous: None, group: 0 }),
    }
}

#[verifier::spinoff_prover]
pub fn producer_acquire_preflight_exec_v1(contents: &ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: &[ProducerReadV1], output: &[Option<ProducerReadReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*contents), requests@.len() <= u64::MAX,
    ensures exact_decision_v1(result, producer_acquire_decision_v1(*contents, consumer, requests@, output@)),
{
    match producer_acquire_header_exec_v1(contents, consumer, requests.len(), output) {
        Err(error) => return Err(error), Ok(()) => {},
    }
    let mut index = 0;
    let mut state = ReadScanV1 { previous: None, group: 0 };
    while index < requests.len()
        invariant index <= requests.len(), state.group <= index,
            requests@.len() <= contents.free@.len(),
            contents.counts@.len() == contents.stable.journal.allocations@.len(),
            producer_acquire_decision_v1(*contents, consumer, requests@, output@)
                == producer_acquire_scan_v1(*contents, consumer, requests@, index as nat, state),
        decreases requests.len() - index,
    {
        match producer_acquire_item_exec_v1(contents, consumer, requests[index], index, state) {
            Err(error) => return Err(error), Ok(next) => state = next,
        }
        index += 1;
    }
    Ok(())
}

pub open spec fn producer_request_count_v1(requests: Seq<ProducerReadV1>, slot: usize) -> nat
    decreases requests.len(),
{
    if requests.len() == 0 { 0 }
    else { producer_request_count_v1(requests.drop_last(), slot)
        + if requests.last().read.allocation.slot == slot { 1nat } else { 0nat } }
}

pub proof fn producer_request_count_bound_v1(requests: Seq<ProducerReadV1>, slot: usize)
    ensures producer_request_count_v1(requests, slot) <= requests.len(),
    decreases requests.len(),
{
    if requests.len() > 0 { producer_request_count_bound_v1(requests.drop_last(), slot); }
}

pub proof fn producer_request_count_push_v1(requests: Seq<ProducerReadV1>, request: ProducerReadV1, slot: usize)
    ensures producer_request_count_v1(requests.push(request), slot)
        == producer_request_count_v1(requests, slot) + if request.read.allocation.slot == slot { 1nat } else { 0nat },
{
    assert(requests.push(request).drop_last() =~= requests);
}

pub open spec fn producer_acquired_reference_v1(before: ProducerReadContentsV1, consumer: WriterKeyV1, index: int) -> ProducerReadReferenceV1 {
    ProducerReadReferenceV1 { slot: before.free@[before.free@.len() - 1 - index],
        incarnation: (before.next_incarnation + index) as u64, consumer }
}

pub open spec fn producer_selected_free_unique_v1(before: ProducerReadContentsV1, count: nat) -> bool {
    forall|i: int, j: int| 0 <= i < j < count ==>
        #[trigger] before.free@[before.free@.len() - 1 - i]
            != #[trigger] before.free@[before.free@.len() - 1 - j]
}

// These are explicit commit safety premises, not an assumed reachable-state invariant.
pub open spec fn producer_acquire_commit_ready_v1(before: ProducerReadContentsV1, requests: Seq<ProducerReadV1>) -> bool {
    &&& requests.len() <= before.free@.len()
    &&& requests.len() <= u64::MAX
    &&& 0 < before.next_incarnation
    &&& before.next_incarnation + requests.len() <= u64::MAX
    &&& producer_selected_free_unique_v1(before, requests.len())
    &&& producer_acquire_prefix_ready_v1(before, requests, requests.len())
}

pub open spec fn producer_acquire_prefix_ready_v1(before: ProducerReadContentsV1, requests: Seq<ProducerReadV1>, end: nat) -> bool {
    forall|i: int| 0 <= i < end ==> {
        let slot = before.free@[before.free@.len() - 1 - i];
        let allocation = (#[trigger] requests[i]).read.allocation.slot;
        &&& slot < before.reservations@.len()
        &&& before.reservations@[slot as int].is_none()
        &&& allocation < before.counts@.len()
        &&& before.counts@[allocation as int]
            + producer_request_count_v1(requests.take(i + 1), allocation) <= usize::MAX
        &&& before.counts@[allocation as int]
            + producer_request_count_v1(requests.take(i + 1), allocation) <= before.reservations@.len()
    }
}

pub open spec fn producer_acquired_reservations_v1(
    before: ProducerReadContentsV1, consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, count: nat,
) -> Seq<Option<ProducerReservationV1>>
    decreases count,
{
    if count == 0 { before.reservations@ }
    else {
        let reference = producer_acquired_reference_v1(before, consumer, count - 1);
        producer_acquired_reservations_v1(before, consumer, requests, (count - 1) as nat)
            .update(reference.slot as int, Some(ProducerReservationV1 { reference, request: requests[count - 1] }))
    }
}

pub open spec fn producer_acquire_commit_prefix_v1(
    before: ProducerReadContentsV1, after: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ProducerReadV1>, output_before: Seq<Option<ProducerReadReferenceV1>>,
    output_after: Seq<Option<ProducerReadReferenceV1>>, count: nat,
) -> bool {
    &&& after.stable == before.stable
    &&& after.next_incarnation == before.next_incarnation
    &&& after.free@ == before.free@.take(before.free@.len() - count)
    &&& after.reservations@ == producer_acquired_reservations_v1(before, consumer, requests, count)
    &&& after.reservations@.len() == before.reservations@.len()
    &&& after.counts@.len() == before.counts@.len()
    &&& forall|a: int| 0 <= a < after.counts@.len() ==>
        after.counts@[a] == before.counts@[a] + producer_request_count_v1(requests.take(count as int), a as usize)
    &&& output_after.len() == output_before.len()
    &&& forall|i: int| 0 <= i < output_after.len() ==> output_after[i]
        == if i < count { Some(producer_acquired_reference_v1(before, consumer, i)) } else { output_before[i] }
}

pub open spec fn producer_acquire_commit_relation_v1(
    before: ProducerReadContentsV1, after: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ProducerReadV1>, output: Seq<Option<ProducerReadReferenceV1>>,
) -> bool {
    &&& after.stable == before.stable
    &&& after.next_incarnation == before.next_incarnation + requests.len()
    &&& after.free@ == before.free@.take(before.free@.len() - requests.len())
    &&& after.reservations@ == producer_acquired_reservations_v1(before, consumer, requests, requests.len())
    &&& after.reservations@.len() == before.reservations@.len()
    &&& after.counts@.len() == before.counts@.len()
    &&& forall|a: int| 0 <= a < after.counts@.len() ==>
        after.counts@[a] == before.counts@[a] + producer_request_count_v1(requests, a as usize)
    &&& output.len() == requests.len()
    &&& forall|i: int| 0 <= i < output.len() ==> output[i] == Some(producer_acquired_reference_v1(before, consumer, i))
}

pub proof fn producer_acquired_reservation_installed_v1(
    before: ProducerReadContentsV1, consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, count: nat, index: nat,
)
    requires producer_acquire_commit_ready_v1(before, requests), index < count <= requests.len(),
    ensures producer_acquired_reservations_v1(before, consumer, requests, count)
        [producer_acquired_reference_v1(before, consumer, index as int).slot as int]
        == Some(ProducerReservationV1 { reference: producer_acquired_reference_v1(before, consumer, index as int), request: requests[index as int] }),
        producer_acquired_reservations_v1(before, consumer, requests, count).len() == before.reservations@.len(),
    decreases count,
{
    if index + 1 < count {
        producer_acquired_reservation_installed_v1(before, consumer, requests, (count - 1) as nat, index);
    } else if count > 1 {
        producer_acquired_reservation_installed_v1(before, consumer, requests, (count - 1) as nat, 0);
    }
}

pub proof fn producer_acquired_outputs_are_installed_v1(
    before: ProducerReadContentsV1, after: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ProducerReadV1>, output: Seq<Option<ProducerReadReferenceV1>>,
)
    requires producer_acquire_commit_ready_v1(before, requests), producer_acquire_commit_relation_v1(before, after, consumer, requests, output),
    ensures forall|i: int| 0 <= i < requests.len() ==> {
        let reference = (#[trigger] output[i]).unwrap();
        &&& output[i].is_some()
        &&& reference.slot < after.reservations@.len()
        &&& after.reservations@[reference.slot as int] == Some(ProducerReservationV1 { reference, request: requests[i] })
        &&& before.next_incarnation <= reference.incarnation < after.next_incarnation
        &&& 0 < reference.incarnation
    },
{
    assert forall|i: int| 0 <= i < requests.len() implies {
        let reference = (#[trigger] output[i]).unwrap();
        &&& output[i].is_some()
        &&& reference.slot < after.reservations@.len()
        &&& after.reservations@[reference.slot as int] == Some(ProducerReservationV1 { reference, request: requests[i] })
        &&& before.next_incarnation <= reference.incarnation < after.next_incarnation
        &&& 0 < reference.incarnation
    } by {
        producer_acquired_reservation_installed_v1(before, consumer, requests, requests.len(), i as nat);
    }
}

pub fn producer_acquire_commit_exec_v1(
    contents: &mut ProducerReadContentsV1, consumer: WriterKeyV1, requests: &[ProducerReadV1],
    output: &mut Vec<Option<ProducerReadReferenceV1>>,
)
    requires producer_acquire_commit_ready_v1(*old(contents), requests@), old(output)@.len() == requests@.len(),
    ensures producer_acquire_commit_relation_v1(*old(contents), *final(contents), consumer, requests@, final(output)@),
{
    let ghost before = *contents;
    let ghost output_before = output@;
    let allocation_capacity = contents.counts.len();
    let mut index = 0;
    proof {
        assert(before.free@.take(before.free@.len() as int) =~= before.free@);
        assert(requests@.take(0) =~= Seq::<ProducerReadV1>::empty());
    }
    while index < requests.len()
        invariant index <= requests.len(), output_before.len() == requests@.len(),
            before.counts@.len() == allocation_capacity,
            producer_acquire_commit_ready_v1(before, requests@),
            producer_acquire_commit_prefix_v1(before, *contents, consumer, requests@, output_before, output@, index as nat),
        decreases requests.len() - index,
    {
        let ghost previous_counts = contents.counts@;
        let ghost previous_output = output@;
        let ghost previous_reservations = contents.reservations@;
        let slot = contents.free.pop().unwrap();
        let reference = ProducerReadReferenceV1 { slot, incarnation: contents.next_incarnation + index as u64, consumer };
        contents.reservations.set(slot, Some(ProducerReservationV1 { reference, request: requests[index] }));
        let allocation = requests[index].read.allocation.slot;
        proof {
            assert(requests@.take(index + 1) =~= requests@.take(index as int).push(requests@[index as int]));
            producer_request_count_push_v1(requests@.take(index as int), requests@[index as int], allocation);
        }
        let count = contents.counts[allocation] + 1;
        contents.counts.set(allocation, count);
        output.set(index, Some(reference));
        proof {
            assert forall|a: int| 0 <= a < contents.counts@.len() implies
                contents.counts@[a] == before.counts@[a] + producer_request_count_v1(requests@.take(index + 1), a as usize) by {
                producer_request_count_push_v1(requests@.take(index as int), requests@[index as int], a as usize);
            }
            assert(contents.free@ =~= before.free@.take(before.free@.len() - index - 1));
        }
        index += 1;
    }
    proof { assert(requests@.take(requests@.len() as int) =~= requests@); }
    contents.next_incarnation += requests.len() as u64;
}

pub proof fn producer_validated_same_slot_local_v1(journal: JournalContentsV1, left: ProducerReadV1, right: ProducerReadV1)
    requires producer_status_decision_v1(journal, left).is_ok(), producer_status_decision_v1(journal, right).is_ok(),
        left.read.allocation.slot == right.read.allocation.slot,
    ensures left.read.allocation.key.local == right.read.allocation.key.local,
{}

pub proof fn producer_count_local_dominance_v1(journal: JournalContentsV1, requests: Seq<ProducerReadV1>,
    keys: Seq<ReadOrderV1>, target: ProducerReadV1)
    requires requests.len() == keys.len(), producer_status_decision_v1(journal, target).is_ok(),
        forall|i: int| 0 <= i < requests.len() ==> producer_status_decision_v1(journal, #[trigger] requests[i]).is_ok(),
        forall|i: int| 0 <= i < requests.len() ==> (#[trigger] keys[i]).0 == requests[i].read.allocation.key.local,
    ensures producer_request_count_v1(requests, target.read.allocation.slot)
        <= order_local_count_v1(keys, target.read.allocation.key.local),
    decreases requests.len(),
{
    if requests.len() > 0 {
        producer_count_local_dominance_v1(journal, requests.drop_last(), keys.drop_last(), target);
        if requests.last().read.allocation.slot == target.read.allocation.slot {
            producer_validated_same_slot_local_v1(journal, requests.last(), target);
        }
    }
}

pub proof fn producer_selected_unique_v1(contents: ProducerReadContentsV1, count: nat)
    requires producer_invariant_v1(contents), count <= contents.free@.len(),
    ensures producer_selected_free_unique_v1(contents, count),
{
    assert forall|i: int, j: int| 0 <= i < j < count implies
        #[trigger] contents.free@[contents.free@.len() - 1 - i]
            != #[trigger] contents.free@[contents.free@.len() - 1 - j] by {
        assert(0 <= contents.free@.len() - 1 - j < contents.free@.len() - 1 - i < contents.free@.len());
    }
}

pub open spec fn producer_acquire_authorized_v1(before: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ProducerReadV1>, end: nat) -> bool
{
    forall|i: int| 0 <= i < end ==> {
        &&& producer_validate_decision_v1(before.stable.journal, #[trigger] requests[i]) == Ok(())
        &&& requests[i].producer.key.local < consumer.local
    }
}

#[verifier::spinoff_prover]
pub proof fn producer_acquire_scan_ready_v1(before: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ProducerReadV1>, output: Seq<Option<ProducerReadReferenceV1>>, index: nat, state: ReadScanV1)
    requires producer_invariant_v1(before), requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        producer_acquire_header_v1(before, consumer, requests.len() as usize, output) == Ok(()),
        producer_selected_free_unique_v1(before, requests.len()),
        index <= requests.len(), producer_acquire_scan_v1(before, consumer, requests, index, state) == Ok(()),
        canonical_prefix_v1(Seq::new(requests.len(), |i: int| read_order_v1(requests[i].read, 0)), index, state),
        producer_acquire_authorized_v1(before, consumer, requests, index),
        producer_acquire_prefix_ready_v1(before, requests, index),
    ensures producer_acquire_commit_ready_v1(before, requests), output.len() == requests.len(),
        producer_acquire_authorized_v1(before, consumer, requests, requests.len()),
    decreases requests.len() - index,
{
    let keys = Seq::new(requests.len(), |i: int| read_order_v1(requests[i].read, 0));
    if index < requests.len() {
        let request = requests[index as int];
        let next = match producer_acquire_item_v1(before, consumer, request, index as usize, state) { Ok(next) => next, Err(_) => state };
        canonical_prefix_extend_v1(keys, index, state);
        producer_count_local_dominance_v1(before.stable.journal, requests.take((index + 1) as int), keys.take((index + 1) as int), request);
        assert(next == ReadScanV1 { previous: Some(keys[index as int]),
            group: next_group_v1(state.previous, state.group, keys[index as int]) });
        assert(canonical_prefix_v1(keys, index + 1, next));
        assert(producer_request_count_v1(requests.take((index + 1) as int), request.read.allocation.slot) <= next.group);
        assert(before.counts@[request.read.allocation.slot as int] + next.group <= before.reservations@.len());
        assert(producer_acquire_prefix_ready_v1(before, requests, index + 1));
        producer_acquire_scan_ready_v1(before, consumer, requests, output, index + 1, next);
    }
}

pub proof fn producer_acquire_preflight_ready_v1(before: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<ProducerReadV1>, output: Seq<Option<ProducerReadReferenceV1>>)
    requires producer_invariant_v1(before), requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        producer_acquire_decision_v1(before, consumer, requests, output) == Ok(()),
    ensures producer_acquire_commit_ready_v1(before, requests), output.len() == requests.len(),
        producer_acquire_authorized_v1(before, consumer, requests, requests.len()),
{
    producer_selected_unique_v1(before, requests.len());
    producer_acquire_scan_ready_v1(before, consumer, requests, output, 0, ReadScanV1 { previous: None, group: 0 });
}

pub proof fn producer_fresh_entry_valid_v1(journal: JournalContentsV1, consumer: WriterKeyV1,
    request: ProducerReadV1, slot: usize, incarnation: u64, next: u64)
    requires pending_custody_v1(journal), consumer.context_generation == journal.context_generation,
        issuable_id_v1(consumer.local), consumer.kind == WriterKindV1::Submission,
        producer_validate_decision_v1(journal, request) == Ok(()), request.producer.key.local < consumer.local,
        0 < incarnation < next,
    ensures producer_entry_valid_v1(journal, ProducerReservationV1 {
        reference: ProducerReadReferenceV1 { slot, incarnation, consumer }, request }, slot as int, next),
{
    producer_status_projection_v1(journal, request);
    assert(producer_status_v1(journal, request) == Some(ProducerStatusV1::Pending));
    assert(request.producer.slot < journal.writers@.len());
    assert(writer_custody_v1(journal, request.producer.slot as int));
    reveal(writer_custody_v1);
    assert(journal.writers@[request.producer.slot as int].is_some());
    assert(writer_key_v1(journal.writers@[request.producer.slot as int].unwrap()).local == request.producer.key.local);
    assert(issuable_id_v1(request.producer.key.local));
    reveal(producer_entry_valid_v1);
}

pub open spec fn producer_acquired_counts_v1(before: ProducerReadContentsV1, requests: Seq<ProducerReadV1>, count: nat) -> Seq<usize> {
    Seq::new(before.counts@.len(), |a: int|
        (before.counts@[a] + producer_request_count_v1(requests.take(count as int), a as usize)) as usize)
}

pub proof fn producer_acquired_counts_step_v1(before: ProducerReadContentsV1, requests: Seq<ProducerReadV1>, count: nat)
    requires before.counts@.len() <= usize::MAX, 0 < count <= requests.len(),
        requests[count - 1].read.allocation.slot < before.counts@.len(),
        before.counts@[requests[count - 1].read.allocation.slot as int]
            + producer_request_count_v1(requests.take(count as int), requests[count - 1].read.allocation.slot) <= usize::MAX,
    ensures producer_acquired_counts_v1(before, requests, (count - 1) as nat)[requests[count - 1].read.allocation.slot as int] < usize::MAX,
        producer_acquired_counts_v1(before, requests, count)
        == producer_acquired_counts_v1(before, requests, (count - 1) as nat).update(
            requests[count - 1].read.allocation.slot as int,
            (producer_acquired_counts_v1(before, requests, (count - 1) as nat)[requests[count - 1].read.allocation.slot as int] + 1) as usize),
{
    let n = (count - 1) as nat;
    let request = requests[n as int];
    let counts = producer_acquired_counts_v1(before, requests, n);
    assert(requests.take(count as int) =~= requests.take(n as int).push(request));
    producer_request_count_push_v1(requests.take(n as int), request, request.read.allocation.slot);
    assert(counts[request.read.allocation.slot as int] < usize::MAX);
    assert forall|a: int| 0 <= a < counts.len() implies #[trigger] producer_acquired_counts_v1(before, requests, count)[a]
        == counts.update(request.read.allocation.slot as int, (counts[request.read.allocation.slot as int] + 1) as usize)[a] by {
        producer_request_count_push_v1(requests.take(n as int), request, a as usize);
    }
    assert(producer_acquired_counts_v1(before, requests, count) =~=
        counts.update(request.read.allocation.slot as int, (counts[request.read.allocation.slot as int] + 1) as usize));
}

#[verifier::spinoff_prover]
pub proof fn producer_acquire_arena_prefix_v1(before: ProducerReadContentsV1, consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, count: nat)
    requires producer_invariant_v1(before), producer_acquire_commit_ready_v1(before, requests), count <= requests.len(),
        consumer.context_generation == before.stable.journal.context_generation, issuable_id_v1(consumer.local),
        consumer.kind == WriterKindV1::Submission,
        forall|i: int| 0 <= i < requests.len() ==> (#[trigger] requests[i]).producer.key.local < consumer.local,
        forall|i: int| 0 <= i < requests.len() ==> producer_validate_decision_v1(before.stable.journal, #[trigger] requests[i]) == Ok(()),
    ensures producer_arena_v1(before.stable.journal, producer_acquired_reservations_v1(before, consumer, requests, count),
        before.free@.take(before.free@.len() - count), producer_acquired_counts_v1(before, requests, count),
        (before.next_incarnation + count) as u64),
    decreases count,
{
    if count == 0 {
        assert(requests.take(0) =~= Seq::<ProducerReadV1>::empty());
        assert(producer_acquired_counts_v1(before, requests, 0) =~= before.counts@);
        assert(before.free@.take(before.free@.len() as int) =~= before.free@);
    } else {
        let n = (count - 1) as nat;
        producer_acquire_arena_prefix_v1(before, consumer, requests, n);
        let reference = producer_acquired_reference_v1(before, consumer, n as int);
        let entry = ProducerReservationV1 { reference, request: requests[n as int] };
        let reservations = producer_acquired_reservations_v1(before, consumer, requests, n);
        let free = before.free@.take(before.free@.len() - n);
        let counts = producer_acquired_counts_v1(before, requests, n);
        assert(requests.take(count as int) =~= requests.take(n as int).push(requests[n as int]));
        producer_request_count_push_v1(requests.take(n as int), entry.request, entry.request.read.allocation.slot);
        producer_fresh_entry_valid_v1(before.stable.journal, consumer, entry.request, reference.slot,
            reference.incarnation, (before.next_incarnation + count) as u64);
        producer_status_projection_v1(before.stable.journal, entry.request);
        producer_acquired_counts_step_v1(before, requests, count);
        producer_arena_insert_v1(before.stable.journal, reservations, free, counts, (before.next_incarnation + n) as u64, entry);
        assert(free.drop_last() =~= before.free@.take(before.free@.len() - count));
    }
}

pub open spec fn producer_contents_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1) -> bool {
    after.stable == before.stable && producer_outer_frame_v1(before, after)
}

pub open spec fn producer_acquire_execution_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, output_before: Seq<Option<ProducerReadReferenceV1>>,
    output_after: Seq<Option<ProducerReadReferenceV1>>, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == producer_acquire_decision_v1(before, consumer, requests, output_before)
    &&& match result {
        Err(_) => producer_contents_frame_v1(before, after) && output_after == output_before,
        Ok(()) => producer_acquire_commit_relation_v1(before, after, consumer, requests, output_after),
    }
}

pub proof fn producer_acquire_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, output_before: Seq<Option<ProducerReadReferenceV1>>,
    output_after: Seq<Option<ProducerReadReferenceV1>>, result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(before), requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        producer_acquire_execution_relation_v1(before, after, consumer, requests, output_before, output_after, result),
    ensures producer_invariant_v1(after), before.next_incarnation <= after.next_incarnation,
        producer_live_reads_v1(after) == producer_live_reads_v1(before) + if result.is_ok() { requests.len() as int } else { 0 },
{
    if result.is_ok() {
        if let Ok(value) = result { assert(value =~= ()); }
        producer_acquire_preflight_ready_v1(before, consumer, requests, output_before);
        producer_acquire_arena_prefix_v1(before, consumer, requests, requests.len());
        assert(requests.take(requests.len() as int) =~= requests);
        assert(after.counts@ =~= producer_acquired_counts_v1(before, requests, requests.len()));
    }
}

#[verifier::spinoff_prover]
pub fn producer_acquire_contents_exec_v1(contents: &mut ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: &[ProducerReadV1], output: &mut Vec<Option<ProducerReadReferenceV1>>) -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*old(contents)), requests@.len() <= u64::MAX,
    ensures producer_acquire_execution_relation_v1(*old(contents), *final(contents), consumer, requests@, old(output)@, final(output)@, result),
{
    let ghost before = *contents;
    let ghost output_before = output@;
    let _count = requests.len();
    let decision = producer_acquire_preflight_exec_v1(contents, consumer, requests, output.as_slice());
    proof { assert(decision == producer_acquire_decision_v1(before, consumer, requests@, output_before)); }
    match decision {
        Err(error) => return Err(error), Ok(value) => { proof { assert(value =~= ()); } },
    }
    proof { producer_acquire_preflight_ready_v1(before, consumer, requests@, output_before); }
    producer_acquire_commit_exec_v1(contents, consumer, requests, output);
    Ok(())
}

pub fn producer_acquire_exec_v1(contents: &mut ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: &[ProducerReadV1], output: &mut Vec<Option<ProducerReadReferenceV1>>) -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*old(contents)), requests@.len() <= u64::MAX,
    ensures producer_acquire_execution_relation_v1(*old(contents), *final(contents), consumer, requests@, old(output)@, final(output)@, result),
        producer_invariant_v1(*final(contents)), old(contents).next_incarnation <= final(contents).next_incarnation,
        producer_live_reads_v1(*final(contents)) == producer_live_reads_v1(*old(contents)) + if result.is_ok() { requests@.len() as int } else { 0 },
{
    let ghost before = *contents;
    let ghost output_before = output@;
    let _count = requests.len();
    let result = producer_acquire_contents_exec_v1(contents, consumer, requests, output);
    proof { producer_acquire_preserves_v1(before, *contents, consumer, requests@, output_before, output@, result); }
    result
}

}

// Producer-read lifecycle candidate: logical execution, not Rust/native refinement.
mod custody {
    include!("context_producer_read_invariant_v1.rs");
    pub use self::issuance::*;
}
use custody::*;
use vstd::prelude::*;

verus! {

pub open spec fn producer_live_reads_v1(contents: ProducerReadContentsV1) -> int {
    contents.stable.leases@.len() - contents.stable.free_reads@.len()
        + contents.reservations@.len() - contents.free@.len()
}

pub proof fn reader_count_addition_fits_usize_v1(contents: ProducerReadContentsV1, allocation: usize)
    requires producer_invariant_v1(contents), allocation < contents.counts@.len(),
    ensures contents.stable.readers@[allocation as int] + contents.counts@[allocation as int] <= 2_097_152,
        contents.stable.readers@[allocation as int] + contents.counts@[allocation as int] <= usize::MAX,
{
    lease_count_bound_v1(contents.stable.leases@, allocation);
    producer_count_bound_v1(contents.reservations@, allocation);
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
pub open spec fn producer_insert_slot_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    entry: ProducerReservationV1) -> bool {
    &&& after.stable == before.stable
    &&& after.reservations@ == before.reservations@.update(entry.reference.slot as int, Some(entry))
    &&& after.free@ == before.free@.drop_last()
    &&& after.counts@ == before.counts@.update(entry.request.read.allocation.slot as int,
        (before.counts@[entry.request.read.allocation.slot as int] + 1) as usize)
    &&& after.next_incarnation == before.next_incarnation + 1
}

pub proof fn producer_insert_slot_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    entry: ProducerReservationV1)
    requires producer_invariant_v1(before), producer_insert_slot_relation_v1(before, after, entry),
        before.free@.len() > 0, before.next_incarnation < u64::MAX,
        entry.reference.slot == before.free@.last(), entry.reference.incarnation == before.next_incarnation,
        producer_entry_valid_v1(before.stable.journal, entry, entry.reference.slot as int, (before.next_incarnation + 1) as u64),
        producer_status_v1(before.stable.journal, entry.request) == Some(ProducerStatusV1::Pending),
        producer_live_reads_v1(before) < before.reservations@.len(),
    ensures producer_invariant_v1(after), producer_live_reads_v1(after) == producer_live_reads_v1(before) + 1,
{
    reveal(producer_entry_valid_v1);
    producer_count_bound_v1(before.reservations@, entry.request.read.allocation.slot);
    producer_arena_insert_v1(before.stable.journal, before.reservations@, before.free@, before.counts@, before.next_incarnation, entry);
}

pub open spec fn producer_remove_slot_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    slot: usize) -> bool {
    &&& after.stable == before.stable
    &&& after.reservations@ == before.reservations@.update(slot as int, None)
    &&& after.free@ == before.free@.push(slot)
    &&& after.counts@ == before.counts@.update(before.reservations@[slot as int].unwrap().request.read.allocation.slot as int,
        (before.counts@[before.reservations@[slot as int].unwrap().request.read.allocation.slot as int] - 1) as usize)
    &&& after.next_incarnation == before.next_incarnation
}

pub proof fn producer_remove_slot_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1, slot: usize)
    requires producer_invariant_v1(before), producer_remove_slot_relation_v1(before, after, slot),
        slot < before.reservations@.len(), before.reservations@[slot as int].is_some(),
        before.free@.len() < before.reservations@.len(),
    ensures producer_invariant_v1(after), producer_live_reads_v1(after) == producer_live_reads_v1(before) - 1,
{
    let entry = before.reservations@[slot as int].unwrap();
    assert(producer_entry_valid_v1(before.stable.journal, entry, slot as int, before.next_incarnation));
    reveal(producer_entry_valid_v1);
    producer_count_zero_v1(before.reservations@, entry.request.read.allocation.slot);
    assert(producer_weight_v1(before.reservations@[slot as int], entry.request.read.allocation.slot) == 1);
    producer_arena_remove_v1(before.stable.journal, before.reservations@, before.free@, before.counts@, before.next_incarnation, slot);
}

pub proof fn producer_chain_frame_v1(before: ReadContentsV1, after: ReadContentsV1, writer: WriterReferenceV1, chain: Seq<usize>)
    requires reader_journal_frame_v1(before, after), retained_chain_v1(before.journal, writer, chain),
    ensures retained_chain_v1(after.journal, writer, chain),
{
    assert forall|i: int| 0 <= i < chain.len() implies #[trigger] chain_link_v1(after.journal, writer, chain, i) by {
        assert(chain_link_v1(before.journal, writer, chain, i));
        reveal(chain_link_v1);
    }
}

pub proof fn producer_custody_frame_v1(before: ReadContentsV1, after: ReadContentsV1)
    requires reader_journal_frame_v1(before, after), pending_custody_v1(before.journal),
    ensures pending_custody_v1(after.journal),
{
    assert forall|a: int| 0 <= a < after.journal.allocations@.len() implies #[trigger] allocation_custody_v1(after.journal, a) by {
        assert(allocation_custody_v1(before.journal, a));
        reveal(allocation_custody_v1);
    }
    assert forall|m: int| 0 <= m < after.journal.members@.len() implies #[trigger] member_custody_v1(after.journal, m) by {
        assert(member_custody_v1(before.journal, m));
        reveal(member_custody_v1);
    }
    assert forall|w: int| 0 <= w < after.journal.writers@.len() implies #[trigger] writer_custody_v1(after.journal, w) by {
        assert(writer_custody_v1(before.journal, w));
        reveal(writer_custody_v1);
        if before.journal.writers@[w].is_some() {
            let entry = before.journal.writers@[w].unwrap();
            if let WriterEntryV1::Reserved(_) = entry { } else {
                let writer = WriterReferenceV1 { slot: w as usize, key: writer_key_v1(entry) };
                let chain = choose|chain: Seq<usize>| retained_chain_v1(before.journal, writer, chain);
                producer_chain_frame_v1(before, after, writer, chain);
            }
        }
    }
}

pub open spec fn producer_outer_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1) -> bool {
    &&& after.reservations@ == before.reservations@
    &&& after.free@ == before.free@
    &&& after.counts@ == before.counts@
    &&& after.next_incarnation == before.next_incarnation
}

pub proof fn producer_arena_journal_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1)
    requires producer_invariant_v1(before), producer_outer_frame_v1(before, after), reader_journal_frame_v1(before.stable, after.stable),
    ensures producer_arena_v1(after.stable.journal, after.reservations@, after.free@, after.counts@, after.next_incarnation),
{
    assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some() implies
        producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
        let entry = before.reservations@[s].unwrap();
        assert(producer_entry_valid_v1(before.stable.journal, entry, s, before.next_incarnation));
        reveal(producer_entry_valid_v1);
    }
}

// Delegate-success composition only; exact wrapper header/shared-budget error order is separate.
pub proof fn stable_acquire_preserves_producer_invariant_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, output_before: Seq<Option<ReadReferenceV1>>,
    output_after: Seq<Option<ReadReferenceV1>>, result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(before), producer_outer_frame_v1(before, after), requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        acquire_execution_relation_v1(before.stable, after.stable, consumer, requests, output_before, output_after, result),
        result.is_ok() ==> producer_live_reads_v1(before) + requests.len() <= before.reservations@.len(),
    ensures producer_invariant_v1(after),
        producer_live_reads_v1(after) == producer_live_reads_v1(before) + if result.is_ok() { requests.len() as int } else { 0 },
{
    acquire_preserves_reader_invariant_v1(before.stable, after.stable, consumer, requests, output_before, output_after, result);
    producer_custody_frame_v1(before.stable, after.stable);
    producer_arena_journal_frame_v1(before, after);
}

pub proof fn stable_release_preserves_producer_invariant_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, references: Seq<ReadReferenceV1>, evidence_consumer: WriterKeyV1, observed_free_capacity: usize,
    result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(before), producer_outer_frame_v1(before, after), references.len() <= usize::MAX,
        release_execution_relation_v1(before.stable, after.stable, consumer, references, evidence_consumer, observed_free_capacity, result),
    ensures producer_invariant_v1(after),
        producer_live_reads_v1(after) == producer_live_reads_v1(before) - if result.is_ok() { references.len() as int } else { 0 },
{
    release_preserves_reader_invariant_v1(before.stable, after.stable, consumer, references, evidence_consumer, observed_free_capacity, result);
    producer_custody_frame_v1(before.stable, after.stable);
    producer_arena_journal_frame_v1(before, after);
}

pub open spec fn stable_wrapper_acquire_header_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    count: usize, output: Seq<Option<ReadReferenceV1>>) -> Result<(), ReadErrorV1> {
    if consumer.context_generation != contents.stable.journal.context_generation { Err(ReadErrorV1::ForeignContext) }
    else if !issuable_id_v1(consumer.local) { Err(ReadErrorV1::InvalidWriterId) }
    else if count == 0 || count != output.len() { Err(ReadErrorV1::RosterCapacity) }
    else if exists|i: int| 0 <= i < output.len() && output[i].is_some() { Err(ReadErrorV1::InvalidState) }
    else if count > contents.reservations@.len() - producer_live_reads_v1(contents) { Err(ReadErrorV1::MemberCapacity) }
    else { capacity_decision_v1(contents.stable, count) }
}

// Logical executable prototype; Rust-source correspondence and physical allocation are separate.
pub fn stable_wrapper_acquire_header_exec_v1(contents: &ProducerReadContentsV1, consumer: WriterKeyV1,
    count: usize, output: &[Option<ReadReferenceV1>]) -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*contents), count <= u64::MAX,
    ensures exact_decision_v1(result, stable_wrapper_acquire_header_v1(*contents, consumer, count, output@)),
{
    if consumer.context_generation != contents.stable.journal.context_generation { return Err(ReadErrorV1::ForeignContext); }
    if !issuable_id_exec_v1(consumer.local) { return Err(ReadErrorV1::InvalidWriterId); }
    if count == 0 || count != output.len() { return Err(ReadErrorV1::RosterCapacity); }
    if !output_vacant_exec_v1(output) { return Err(ReadErrorV1::InvalidState); }
    proof { producer_capacity_arithmetic_v1(*contents); }
    let stable_live = contents.stable.leases.len() - contents.stable.free_reads.len();
    let producer_live = contents.reservations.len() - contents.free.len();
    let remaining = contents.reservations.len() - (stable_live + producer_live);
    if count > remaining { return Err(ReadErrorV1::MemberCapacity); }
    validate_capacity_exec_v1(&contents.stable, count)
}

pub open spec fn stable_wrapper_acquire_decision_v1(contents: ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: Seq<AllocationReadV1>, output: Seq<Option<ReadReferenceV1>>) -> Result<(), ReadErrorV1> {
    match stable_wrapper_acquire_header_v1(contents, consumer, requests.len() as usize, output) {
        Err(error) => Err(error),
        Ok(()) => acquire_decision_v1(contents.stable, consumer, requests, output),
    }
}

pub open spec fn stable_wrapper_acquire_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, output_before: Seq<Option<ReadReferenceV1>>,
    output_after: Seq<Option<ReadReferenceV1>>, result: Result<(), ReadErrorV1>) -> bool {
    &&& producer_outer_frame_v1(before, after)
    &&& result == stable_wrapper_acquire_decision_v1(before, consumer, requests, output_before)
    &&& match stable_wrapper_acquire_header_v1(before, consumer, requests.len() as usize, output_before) {
        Err(_) => reader_contents_frame_v1(before.stable, after.stable) && output_after == output_before,
        Ok(()) => acquire_execution_relation_v1(before.stable, after.stable, consumer, requests, output_before, output_after, result),
    }
}

pub fn stable_wrapper_acquire_exec_v1(contents: &mut ProducerReadContentsV1, consumer: WriterKeyV1,
    requests: &[AllocationReadV1], output: &mut Vec<Option<ReadReferenceV1>>) -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*old(contents)), requests@.len() <= u64::MAX,
    ensures stable_wrapper_acquire_relation_v1(*old(contents), *final(contents), consumer, requests@, old(output)@, final(output)@, result),
        producer_invariant_v1(*final(contents)),
{
    let ghost before = *contents;
    let ghost output_before = output@;
    match stable_wrapper_acquire_header_exec_v1(contents, consumer, requests.len(), output.as_slice()) {
        Err(error) => return Err(error),
        Ok(_) => {},
    }
    let result = acquire_verified_v1(&mut contents.stable, consumer, requests, output);
    proof {
        stable_acquire_preserves_producer_invariant_v1(before, *contents, consumer, requests@, output_before, output@, result);
    }
    result
}

pub fn stable_wrapper_header_order_witness_v1() -> (result: bool)
    ensures result,
{
    let mut contents = match producer_nonempty_witness_v1() { Some(value) => value, None => return false };
    contents.stable.next_incarnation = u64::MAX;
    proof {
        assert(reader_invariant_v1(contents.stable));
        assert(producer_invariant_v1(contents));
        assert(producer_live_reads_v1(contents) == contents.reservations@.len());
    }
    let mut output = Vec::new();
    output.push(None);
    let consumer = WriterKeyV1 { context_generation: contents.stable.journal.context_generation, local: 30, kind: WriterKindV1::Submission };
    let capacity = stable_wrapper_acquire_header_exec_v1(&contents, consumer, 1, output.as_slice());
    proof { assert(capacity == Err(ReadErrorV1::MemberCapacity)); }
    let invalid = WriterKeyV1 { local: 0, ..consumer };
    let foreign = WriterKeyV1 { context_generation: 0, ..invalid };
    let context = stable_wrapper_acquire_header_exec_v1(&contents, foreign, 1, output.as_slice());
    proof { assert(context == Err(ReadErrorV1::ForeignContext)); }
    let identity = stable_wrapper_acquire_header_exec_v1(&contents, invalid, 1, output.as_slice());
    proof { assert(identity == Err(ReadErrorV1::InvalidWriterId)); }
    let empty = stable_wrapper_acquire_header_exec_v1(&contents, consumer, 0, output.as_slice());
    proof { assert(empty == Err(ReadErrorV1::RosterCapacity)); }
    output.set(0, Some(ReadReferenceV1 { slot: 0, incarnation: 1, consumer }));
    proof { assert(output@[0].is_some()); }
    let dirty = stable_wrapper_acquire_header_exec_v1(&contents, consumer, 1, output.as_slice());
    proof { assert(dirty == Err(ReadErrorV1::InvalidState)); }
    let empty_contents = match producer_constructor_exec_v1(7, 1, 1, 1) { Ok(value) => value, Err(_) => return false };
    let synchronous = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Synchronous };
    output.set(0, None);
    let accepted = stable_wrapper_acquire_header_exec_v1(&empty_contents, synchronous, 1, output.as_slice());
    proof { assert(accepted == Ok(())); }
    true
}

}

verus! {

pub fn same_producer_exec_v1(left: WriterReferenceV1, right: WriterReferenceV1) -> (result: bool)
    ensures result == same_producer_v1(left, right),
{
    left.slot == right.slot && same_key_exec_v1(left.key, right.key)
}

pub fn producer_writer_key_exec_v1(entry: WriterEntryV1) -> (result: WriterKeyV1)
    ensures result == writer_key_v1(entry),
{
    match entry {
        WriterEntryV1::Reserved(key) => key,
        WriterEntryV1::Pending { key, .. } => key,
        WriterEntryV1::Unknown { key, .. } => key,
    }
}

pub open spec fn producer_status_decision_v1(journal: JournalContentsV1, request: ProducerReadV1)
    -> Result<ProducerStatusV1, ReadErrorV1>
{
    match allocation_decision_v1(journal, request.read.allocation) {
        Err(error) => Err(error),
        Ok(entry) => {
            let read = request.read;
            if entry.device != read.device { Err(ReadErrorV1::AllocationDeviceMismatch) }
            else if entry.byte_extent != read.byte_extent { Err(ReadErrorV1::AllocationExtentMismatch) }
            else if read.byte_len == 0 || read.byte_offset + read.byte_len > u64::MAX
                || read.byte_offset + read.byte_len > read.byte_extent { Err(ReadErrorV1::InvalidExtent) }
            else if request.producer.key.context_generation != journal.context_generation
                || request.producer.key.kind != WriterKindV1::Submission
                || read.content_lineage >= read.attempt_epoch
                || entry.attempt_epoch != read.attempt_epoch { Err(ReadErrorV1::InvalidState) }
            else { match entry.pending_member {
                Some(slot) => {
                    let member = journal.members@[slot as int].unwrap();
                    if !same_producer_v1(member.writer, request.producer)
                        || entry.content_lineage != read.content_lineage { Err(ReadErrorV1::InvalidState) }
                    else if request.producer.slot >= journal.writers@.len() { Err(ReadErrorV1::InvalidReference) }
                    else { match journal.writers@[request.producer.slot as int] {
                        None => Err(ReadErrorV1::InvalidReference),
                        Some(writer) => {
                            let key = writer_key_v1(writer);
                            if !same_key_v1(key, request.producer.key) || key.context_generation != journal.context_generation {
                                Err(ReadErrorV1::InvalidReference)
                            } else { match writer {
                                WriterEntryV1::Reserved(_) => Err(ReadErrorV1::InvalidState),
                                WriterEntryV1::Pending { .. } => Ok(ProducerStatusV1::Pending),
                                WriterEntryV1::Unknown { .. } => Ok(ProducerStatusV1::Unknown),
                            } }
                        },
                    } }
                },
                None => if entry.content_lineage == read.attempt_epoch { Ok(ProducerStatusV1::Success) }
                    else if entry.content_lineage == read.content_lineage { Ok(ProducerStatusV1::NoEffect) }
                    else { Err(ReadErrorV1::InvalidState) },
            } }
        },
    }
}

pub proof fn producer_status_projection_v1(journal: JournalContentsV1, request: ProducerReadV1)
    ensures producer_status_v1(journal, request) == match producer_status_decision_v1(journal, request) {
        Ok(status) => Some(status), Err(_) => None,
    },
{}

#[verifier::spinoff_prover]
pub fn producer_status_exec_v1(journal: &JournalContentsV1, request: ProducerReadV1)
    -> (result: Result<ProducerStatusV1, ReadErrorV1>)
    ensures exact_decision_v1(result, producer_status_decision_v1(*journal, request)),
{
    let read = request.read;
    let entry = match allocation_lookup_exec_v1(journal, read.allocation) {
        Ok(value) => value, Err(error) => return Err(error),
    };
    if entry.device.context_generation != read.device.context_generation || entry.device.local != read.device.local {
        return Err(ReadErrorV1::AllocationDeviceMismatch);
    }
    if entry.byte_extent != read.byte_extent { return Err(ReadErrorV1::AllocationExtentMismatch); }
    if read.byte_len == 0 { return Err(ReadErrorV1::InvalidExtent); }
    let end = match read.byte_offset.checked_add(read.byte_len) {
        Some(value) => value, None => return Err(ReadErrorV1::InvalidExtent),
    };
    if end > read.byte_extent { return Err(ReadErrorV1::InvalidExtent); }
    if request.producer.key.context_generation != journal.context_generation
        || !matches!(request.producer.key.kind, WriterKindV1::Submission)
        || read.content_lineage >= read.attempt_epoch || entry.attempt_epoch != read.attempt_epoch {
        return Err(ReadErrorV1::InvalidState);
    }
    match entry.pending_member {
        Some(slot) => {
            let member = journal.members[slot].unwrap();
            if !same_producer_exec_v1(member.writer, request.producer) || entry.content_lineage != read.content_lineage {
                return Err(ReadErrorV1::InvalidState);
            }
            if request.producer.slot >= journal.writers.len() { return Err(ReadErrorV1::InvalidReference); }
            let writer = match journal.writers[request.producer.slot] {
                Some(value) => value, None => return Err(ReadErrorV1::InvalidReference),
            };
            let key = producer_writer_key_exec_v1(writer);
            if !same_key_exec_v1(key, request.producer.key) || key.context_generation != journal.context_generation {
                return Err(ReadErrorV1::InvalidReference);
            }
            match writer {
                WriterEntryV1::Reserved(_) => Err(ReadErrorV1::InvalidState),
                WriterEntryV1::Pending { .. } => Ok(ProducerStatusV1::Pending),
                WriterEntryV1::Unknown { .. } => Ok(ProducerStatusV1::Unknown),
            }
        },
        None => if entry.content_lineage == read.attempt_epoch { Ok(ProducerStatusV1::Success) }
            else if entry.content_lineage == read.content_lineage { Ok(ProducerStatusV1::NoEffect) }
            else { Err(ReadErrorV1::InvalidState) },
    }
}

pub open spec fn producer_validate_decision_v1(journal: JournalContentsV1, request: ProducerReadV1) -> Result<(), ReadErrorV1> {
    match producer_status_decision_v1(journal, request) {
        Err(error) => Err(error),
        Ok(ProducerStatusV1::Pending) => Ok(()),
        Ok(_) => Err(ReadErrorV1::AllocationBusy),
    }
}

#[verifier::spinoff_prover]
pub fn producer_validate_exec_v1(journal: &JournalContentsV1, request: ProducerReadV1) -> (result: Result<(), ReadErrorV1>)
    ensures exact_decision_v1(result, producer_validate_decision_v1(*journal, request)),
{
    match producer_status_exec_v1(journal, request) {
        Err(error) => Err(error),
        Ok(ProducerStatusV1::Pending) => Ok(()),
        Ok(_) => Err(ReadErrorV1::AllocationBusy),
    }
}

pub open spec fn same_producer_read_reference_v1(left: ProducerReadReferenceV1, right: ProducerReadReferenceV1) -> bool {
    left.slot == right.slot && left.incarnation == right.incarnation && same_key_v1(left.consumer, right.consumer)
}

pub fn same_producer_read_reference_exec_v1(left: ProducerReadReferenceV1, right: ProducerReadReferenceV1) -> (result: bool)
    ensures result == same_producer_read_reference_v1(left, right),
{
    left.slot == right.slot && left.incarnation == right.incarnation && same_key_exec_v1(left.consumer, right.consumer)
}

pub open spec fn producer_lookup_decision_v1(contents: ProducerReadContentsV1, reference: ProducerReadReferenceV1)
    -> Result<ProducerReadV1, ReadErrorV1>
{
    if reference.slot >= contents.reservations@.len() { Err(ReadErrorV1::InvalidReference) }
    else { match contents.reservations@[reference.slot as int] {
        None => Err(ReadErrorV1::InvalidReference),
        Some(entry) => {
            if !same_producer_read_reference_v1(entry.reference, reference) { Err(ReadErrorV1::InvalidReference) }
            else { match producer_status_decision_v1(contents.stable.journal, entry.request) {
                Err(error) => Err(error), Ok(_) => Ok(entry.request),
            } }
        },
    } }
}

pub fn producer_lookup_exec_v1(contents: &ProducerReadContentsV1, reference: ProducerReadReferenceV1)
    -> (result: Result<ProducerReadV1, ReadErrorV1>)
    ensures exact_decision_v1(result, producer_lookup_decision_v1(*contents, reference)),
{
    if reference.slot >= contents.reservations.len() { return Err(ReadErrorV1::InvalidReference); }
    let entry = match contents.reservations[reference.slot] {
        Some(value) => value, None => return Err(ReadErrorV1::InvalidReference),
    };
    if !same_producer_read_reference_exec_v1(entry.reference, reference) { return Err(ReadErrorV1::InvalidReference); }
    match producer_status_exec_v1(&contents.stable.journal, entry.request) {
        Err(error) => Err(error), Ok(_) => Ok(entry.request),
    }
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
            Err(_) => return Ok(()), Ok(next) => state = next,
        }
        index += 1;
    }
    Ok(())
}

}

verus! {

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

}

verus! {

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

}

verus! {

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

}

verus! {

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

pub fn stable_wrapper_release_exec_v1(contents: &mut ProducerReadContentsV1, consumer: WriterKeyV1,
    references: &[ReadReferenceV1], evidence_consumer: WriterKeyV1, observed_free_capacity: usize)
    -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*old(contents)),
    ensures producer_outer_frame_v1(*old(contents), *final(contents)),
        release_execution_relation_v1(old(contents).stable, final(contents).stable, consumer, references@, evidence_consumer, observed_free_capacity, result),
        producer_invariant_v1(*final(contents)),
        producer_live_reads_v1(*final(contents)) == producer_live_reads_v1(*old(contents)) - if result.is_ok() { references@.len() as int } else { 0 },
{
    let ghost before = *contents;
    let _count = references.len();
    let result = release_verified_v1(&mut contents.stable, consumer, references, evidence_consumer, observed_free_capacity);
    proof {
        stable_release_preserves_producer_invariant_v1(before, *contents, consumer, references@, evidence_consumer, observed_free_capacity, result);
    }
    result
}

pub open spec fn producer_reader_count_decision_v1(contents: ProducerReadContentsV1, allocation: AllocationReferenceV1)
    -> Result<usize, ReadErrorV1>
{
    match allocation_decision_v1(contents.stable.journal, allocation) {
        Err(error) => Err(error),
        Ok(_) => Ok((contents.stable.readers@[allocation.slot as int] + contents.counts@[allocation.slot as int]) as usize),
    }
}

pub fn producer_reader_count_exec_v1(contents: &ProducerReadContentsV1, allocation: AllocationReferenceV1)
    -> (result: Result<usize, ReadErrorV1>)
    requires producer_invariant_v1(*contents),
    ensures exact_decision_v1(result, producer_reader_count_decision_v1(*contents, allocation)),
{
    match allocation_lookup_exec_v1(&contents.stable.journal, allocation) {
        Err(error) => return Err(error), Ok(_) => {},
    }
    proof { reader_count_addition_fits_usize_v1(*contents, allocation.slot); }
    Ok(contents.stable.readers[allocation.slot] + contents.counts[allocation.slot])
}

pub open spec fn producer_unread_scan_v1(contents: ProducerReadContentsV1, allocations: Seq<AllocationReferenceV1>, index: nat)
    -> Result<(), ReadErrorV1>
    decreases allocations.len() - index,
{
    if index >= allocations.len() { Ok(()) }
    else { match producer_reader_count_decision_v1(contents, allocations[index as int]) {
        Err(error) => Err(error),
        Ok(count) => if count != 0 { Err(ReadErrorV1::AllocationBusy) } else { producer_unread_scan_v1(contents, allocations, index + 1) },
    } }
}

pub fn producer_require_unread_exec_v1(contents: &ProducerReadContentsV1, allocations: &[AllocationReferenceV1])
    -> (result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(*contents),
    ensures exact_decision_v1(result, producer_unread_scan_v1(*contents, allocations@, 0)),
{
    let mut index = 0;
    while index < allocations.len()
        invariant index <= allocations.len(), producer_invariant_v1(*contents),
            producer_unread_scan_v1(*contents, allocations@, 0) == producer_unread_scan_v1(*contents, allocations@, index as nat),
        decreases allocations.len() - index,
    {
        let count = match producer_reader_count_exec_v1(contents, allocations[index]) {
            Err(error) => return Err(error), Ok(value) => value,
        };
        if count != 0 { return Err(ReadErrorV1::AllocationBusy); }
        index += 1;
    }
    Ok(())
}

pub proof fn producer_retained_unread_rejection_v1(contents: ProducerReadContentsV1, slot: usize)
    requires producer_invariant_v1(contents), slot < contents.reservations@.len(), contents.reservations@[slot as int].is_some(),
    ensures producer_unread_scan_v1(contents, seq![contents.reservations@[slot as int].unwrap().request.read.allocation], 0)
        == Err(ReadErrorV1::AllocationBusy),
{
    retained_producer_positive_count_v1(contents, slot);
    let entry = contents.reservations@[slot as int].unwrap();
    assert(producer_entry_valid_v1(contents.stable.journal, entry, slot as int, contents.next_incarnation));
    reveal(producer_entry_valid_v1);
    reader_count_addition_fits_usize_v1(contents, entry.request.read.allocation.slot);
}

}

verus! {

// Populating a logical pending writer is a fixture, not begin-write refinement.
pub fn producer_pending_fixture_v1() -> (result: Option<(ProducerReadContentsV1, ProducerReadV1)>)
    ensures result.is_some(),
        producer_invariant_v1(result.unwrap().0),
        result.unwrap().0.stable.journal.context_generation == 7,
        result.unwrap().0.reservations@.len() == 4,
        result.unwrap().0.free@ == seq![3usize, 2usize, 1usize, 0usize],
        result.unwrap().0.counts@ == seq![0usize], result.unwrap().0.next_incarnation == 1,
        producer_live_reads_v1(result.unwrap().0) == 0,
        producer_validate_decision_v1(result.unwrap().0.stable.journal, result.unwrap().1) == Ok(()),
        result.unwrap().0.stable.journal.allocations@.len() == 1,
        result.unwrap().0.stable.journal.members@.len() == 1,
        result.unwrap().0.stable.journal.free@.len() == 0,
        result.unwrap().0.stable.journal.member_free@.len() == 0,
        result.unwrap().0.stable.journal.writers@ == seq![Some(WriterEntryV1::Pending {
            key: result.unwrap().1.producer.key, head: Some(0usize), count: 1usize })],
        retained_chain_v1(result.unwrap().0.stable.journal, result.unwrap().1.producer, seq![0usize]),
        result.unwrap().1.producer.slot == 0,
        result.unwrap().1.producer.key.local == 10,
        result.unwrap().1.read.allocation.slot == 0,
        result.unwrap().1.read.byte_extent == 16,
        result.unwrap().1.read.byte_offset == 0, result.unwrap().1.read.byte_len == 8,
{
    let mut contents = match producer_constructor_exec_v1(7, 1, 1, 4) { Ok(value) => value, Err(_) => return None };
    let ghost empty = contents.stable;
    let key = AllocationKeyV1 { context_generation: 7, local: 1 };
    let device = DeviceKeyV1 { context_generation: 7, local: 2 };
    let allocation = AllocationReferenceV1 { slot: 0, key };
    let producer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let _allocation_slot = contents.stable.journal.allocation_free.pop();
    let _member_slot = contents.stable.journal.member_free.pop();
    let _writer_slot = contents.stable.journal.free.pop();
    contents.stable.journal.registration_watermark = 10;
    contents.stable.journal.allocations.set(0, Some(AllocationEntryV1 {
        key, device, byte_extent: 16, attempt_epoch: 1, content_lineage: 0, pending_member: Some(0),
    }));
    contents.stable.journal.members.set(0, Some(MemberEntryV1 {
        writer: producer, allocation, prior_lineage: 0, attempt_epoch: 1, next: None,
    }));
    contents.stable.journal.writers.set(0, Some(WriterEntryV1::Pending { key: producer.key, head: Some(0), count: 1 }));
    let request = ProducerReadV1 { read: AllocationReadV1 {
        allocation, device, byte_extent: 16, byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0,
    }, producer };
    proof {
        reader_allocation_frame_preserves_v1(empty, contents.stable);
        reveal(chain_link_v1);
        assert(chain_link_v1(contents.stable.journal, producer, seq![0usize], 0));
        assert forall|m: int| 0 <= m < contents.stable.journal.members@.len()
            && (#[trigger] contents.stable.journal.members@[m]).is_some()
            && same_producer_v1(contents.stable.journal.members@[m].unwrap().writer, producer)
            implies seq![0usize].contains(m as usize) by { assert(m == 0); }
        assert(retained_chain_v1(contents.stable.journal, producer, seq![0usize]));
        reveal(allocation_custody_v1);
        reveal(member_custody_v1);
        reveal(writer_custody_v1);
        assert(pending_custody_v1(contents.stable.journal));
        assert(producer_arena_v1(contents.stable.journal, contents.reservations@, contents.free@, contents.counts@, contents.next_incarnation));
        assert(contents.free@ =~= seq![3usize, 2usize, 1usize, 0usize]);
    }
    Some((contents, request))
}

pub fn producer_late_rejection_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut contents, first) = match producer_pending_fixture_v1() { Some(value) => value, None => return false };
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let invalid = ProducerReadV1 { read: AllocationReadV1 { byte_offset: 16, ..first.read }, ..first };
    let mut requests = Vec::new();
    requests.push(first);
    requests.push(invalid);
    let mut output = Vec::new();
    output.push(None);
    output.push(None);
    let ghost before = contents;
    let ghost output_before = output@;
    let rejected = producer_acquire_exec_v1(&mut contents, consumer, requests.as_slice(), &mut output);
    proof {
        reveal_with_fuel(producer_acquire_scan_v1, 3);
        assert(rejected == Err(ReadErrorV1::InvalidExtent));
        assert(producer_contents_frame_v1(before, contents));
        assert(output@ == output_before);
    }
    let synchronous = WriterKeyV1 { kind: WriterKindV1::Synchronous, ..consumer };
    let kind = producer_acquire_header_exec_v1(&contents, synchronous, requests.len(), output.as_slice());
    proof { assert(kind == Err(ReadErrorV1::InvalidWriterId)); }
    contents.next_incarnation = u64::MAX;
    proof { assert(producer_invariant_v1(contents)); }
    let capacity = producer_capacity_exec_v1(&contents, 5);
    proof { assert(capacity == Err(ReadErrorV1::MemberCapacity)); }
    let epoch = producer_capacity_exec_v1(&contents, 1);
    proof { assert(epoch == Err(ReadErrorV1::EpochExhausted)); }
    true
}

pub fn producer_batch_roundtrip_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut contents, first) = match producer_pending_fixture_v1() { Some(value) => value, None => return false };
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let second = ProducerReadV1 { read: AllocationReadV1 { byte_offset: 8, ..first.read }, ..first };
    let mut requests = Vec::new();
    requests.push(first);
    requests.push(second);
    let mut output = Vec::new();
    output.push(None);
    output.push(None);
    let acquired = producer_acquire_exec_v1(&mut contents, consumer, requests.as_slice(), &mut output);
    proof {
        reveal_with_fuel(producer_acquire_scan_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
        assert(acquired == Ok(()));
        assert(contents.free@ == seq![3usize, 2usize]);
        assert(contents.counts@ == seq![2usize]);
        assert(contents.next_incarnation == 3);
        assert(producer_live_reads_v1(contents) == 2);
        assert(output@[0] == Some(ProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer }));
        assert(output@[1] == Some(ProducerReadReferenceV1 { slot: 1, incarnation: 2, consumer }));
    }
    let first_reference = output[0].unwrap();
    let second_reference = output[1].unwrap();
    let mut references = Vec::new();
    references.push(first_reference);
    references.push(second_reference);
    let ghost before_late = contents;
    references.set(1, ProducerReadReferenceV1 { incarnation: 7, ..second_reference });
    let late = producer_release_exec_v1(&mut contents, consumer, references.as_slice(), consumer, 4);
    proof {
        reveal_with_fuel(producer_acquired_reservations_v1, 3);
        reveal_with_fuel(producer_release_scan_v1, 3);
        assert(late == Err(ReadErrorV1::InvalidReference));
        assert(producer_contents_frame_v1(before_late, contents));
    }
    references.set(1, second_reference);
    let ghost before_capacity = contents;
    let short = producer_release_exec_v1(&mut contents, consumer, references.as_slice(), consumer, 3);
    proof {
        assert(short == Err(ReadErrorV1::InvalidState));
        assert(producer_contents_frame_v1(before_capacity, contents));
    }
    let released = producer_release_exec_v1(&mut contents, consumer, references.as_slice(), consumer, 4);
    proof {
        reveal_with_fuel(producer_acquired_reservations_v1, 3);
        reveal_with_fuel(producer_release_scan_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
        assert(released == Ok(()));
        assert(contents.free@ == seq![3usize, 2usize, 0usize, 1usize]);
        assert(contents.counts@ == seq![0usize]);
        assert(contents.next_incarnation == 3);
        assert(producer_live_reads_v1(contents) == 0);
    }
    requests.pop();
    output.clear();
    output.push(None);
    let reused = producer_acquire_exec_v1(&mut contents, consumer, requests.as_slice(), &mut output);
    proof {
        reveal_with_fuel(producer_acquire_scan_v1, 2);
        assert(reused == Ok(()));
        assert(output@[0] == Some(ProducerReadReferenceV1 { slot: 1, incarnation: 3, consumer }));
    }
    let stale = producer_lookup_exec_v1(&contents, second_reference);
    proof {
        reveal_with_fuel(producer_acquired_reservations_v1, 2);
        assert(stale == Err(ReadErrorV1::InvalidReference));
    }
    let mut allocations = Vec::new();
    allocations.push(first.read.allocation);
    let busy = producer_require_unread_exec_v1(&contents, allocations.as_slice());
    proof {
        producer_retained_unread_rejection_v1(contents, 1);
        assert(busy == Err(ReadErrorV1::AllocationBusy));
    }
    true
}

// Each status is witnessed from a fresh pending fixture with a retained reader.
pub fn producer_status_release_witness_v1(status: ProducerStatusV1) -> (result: bool)
    ensures result,
{
    let (mut contents, request) = match producer_pending_fixture_v1() { Some(value) => value, None => return false };
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let mut requests = Vec::new();
    requests.push(request);
    let mut output = Vec::new();
    output.push(None);
    let acquired = producer_acquire_exec_v1(&mut contents, consumer, requests.as_slice(), &mut output);
    proof {
        reveal_with_fuel(producer_acquire_scan_v1, 2);
        assert(acquired == Ok(()));
        assert(output@[0] == Some(ProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer }));
    }
    let ghost before = contents;
    match status {
        ProducerStatusV1::Pending => {},
        ProducerStatusV1::Unknown => {
            contents.stable.journal.writers.set(0, Some(WriterEntryV1::Unknown {
                key: request.producer.key, head: Some(0), count: 1,
            }));
            proof {
                assert(mark_unknown_relation_v1(before.stable.journal, contents.stable.journal, request.producer));
                unknown_preserves_producer_invariant_v1(before, contents, request.producer);
            }
        },
        ProducerStatusV1::Success | ProducerStatusV1::NoEffect => {
            let success = matches!(status, ProducerStatusV1::Success);
            let entry = contents.stable.journal.allocations[0].unwrap();
            contents.stable.journal.allocations.set(0, Some(AllocationEntryV1 {
                content_lineage: if success { entry.attempt_epoch } else { entry.content_lineage },
                pending_member: None, ..entry
            }));
            contents.stable.journal.members.set(0, None);
            contents.stable.journal.member_free.push(0);
            contents.stable.journal.writers.set(0, None);
            contents.stable.journal.free.push(0);
            proof {
                assert(contents.stable.journal.member_free@ =~= before.stable.journal.member_free@ + seq![0usize]);
                assert(contents.stable.journal.members@ =~= Seq::new(before.stable.journal.members@.len(), |m: int|
                    if seq![0usize].contains(m as usize) { None } else { before.stable.journal.members@[m] }));
                assert(contents.stable.journal.allocations@ =~= Seq::new(before.stable.journal.allocations@.len(), |a: int|
                    if selected_allocation_v1(before.stable.journal, seq![0usize], a) {
                        Some(settled_allocation_v1(before.stable.journal.allocations@[a].unwrap(), success))
                    } else { before.stable.journal.allocations@[a] }));
                assert(settle_chain_relation_v1(before.stable.journal, contents.stable.journal, request.producer, seq![0usize], success));
                settle_preserves_producer_invariant_v1(before, contents, request.producer, seq![0usize], success);
            }
        },
    }
    let observed = producer_status_exec_v1(&contents.stable.journal, request);
    proof { assert(observed == Ok(status)); }
    let mut references = Vec::new();
    references.push(output[0].unwrap());
    let ghost journal_before_release = contents.stable.journal;
    let released = producer_release_exec_v1(&mut contents, consumer, references.as_slice(), consumer, 4);
    proof {
        reveal_with_fuel(producer_acquired_reservations_v1, 2);
        reveal_with_fuel(producer_release_scan_v1, 2);
        reveal_with_fuel(producer_request_count_v1, 2);
        assert(released == Ok(()));
        assert(contents.free@ == seq![3usize, 2usize, 1usize, 0usize]);
        assert(contents.counts@ == seq![0usize]);
        assert(contents.next_incarnation == 2);
        assert(producer_live_reads_v1(contents) == 0);
        assert(contents.stable.journal == journal_before_release);
    }
    true
}

}

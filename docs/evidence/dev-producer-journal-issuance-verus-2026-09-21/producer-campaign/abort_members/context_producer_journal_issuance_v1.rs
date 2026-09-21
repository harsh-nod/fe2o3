// Producer journal issuance composition: logical contents, not Rust/native refinement.
include!("context_producer_read_invariant_v1.rs");

mod journal_issuance {
use super::*;
use super::issuance::*;

verus! {

pub open spec fn issued_custody_v1(journal: JournalContentsV1, storage: StorageCapacitiesV1,
    history: Seq<WriterReferenceV1>) -> bool
{
    pending_custody_v1(journal)
        && issuance_invariant_v1(issuance_contents_projection_v1(journal, storage, history))
}

pub open spec fn issued_producer_v1(contents: ProducerReadContentsV1, storage: StorageCapacitiesV1,
    history: Seq<WriterReferenceV1>) -> bool
{
    producer_invariant_v1(contents)
        && issuance_invariant_v1(issuance_contents_projection_v1(contents.stable.journal, storage, history))
}

pub open spec fn registration_history_v1(history: Seq<WriterReferenceV1>,
    result: Result<WriterReferenceV1, JournalErrorV1>) -> Seq<WriterReferenceV1>
{
    match result { Ok(reference) => history.push(reference), Err(_) => history }
}

pub proof fn issued_writer_identity_v1(state: JournalStateV1, slot: int)
    requires issuance_invariant_v1(state), 0 <= slot < state.writers.len(), state.writers[slot].is_some(),
    ensures writer_key_v1(state.writers[slot].unwrap()).context_generation == state.context_generation,
        issuable_id_v1(writer_key_v1(state.writers[slot].unwrap()).local),
        writer_key_v1(state.writers[slot].unwrap()).local <= state.registration_watermark,
{
    let key = writer_key_v1(state.writers[slot].unwrap());
    let history = state.successful_registrations;
    let i = choose|i: int| 0 <= i < history.len() && history[i].slot == slot && same_key_v1(history[i].key, key);
    history_key_bounded_v1(state, i);
}

pub proof fn issued_writer_unique_v1(state: JournalStateV1, left: int, right: int)
    requires issuance_invariant_v1(state), 0 <= left < right < state.writers.len(),
        state.writers[left].is_some(), state.writers[right].is_some(),
    ensures writer_key_v1(state.writers[left].unwrap()).local != writer_key_v1(state.writers[right].unwrap()).local,
{
    let history = state.successful_registrations;
    let a = writer_key_v1(state.writers[left].unwrap());
    let b = writer_key_v1(state.writers[right].unwrap());
    let i = choose|i: int| 0 <= i < history.len() && history[i].slot == left && same_key_v1(history[i].key, a);
    let j = choose|j: int| 0 <= j < history.len() && history[j].slot == right && same_key_v1(history[j].key, b);
    assert(i != j);
    if i < j { assert(history[i].key.local < history[j].key.local); }
    else { assert(history[j].key.local < history[i].key.local); }
}

pub proof fn issued_writer_partition_v1(state: JournalStateV1)
    requires issuance_invariant_v1(state),
    ensures slot_partition_v1(state.writers, state.free),
{
    prefix_bounds_v1(state.writers, state.writers.len() as int);
    assert(state.free.no_duplicates());
}

pub proof fn issued_constructor_v1(journal: JournalContentsV1, context: u64, allocations: usize,
    writers: usize, storage: StorageCapacitiesV1)
    requires constructor_contents_relation_v1(context, allocations, writers, Ok(journal)),
        storage_admission_v1(allocations, writers, storage),
    ensures issued_custody_v1(journal, storage, Seq::empty()),
{
    constructor_contents_refines_v1(context, allocations, writers, Ok(journal), storage);
    producer_empty_custody_v1(journal, context, allocations, writers);
}

pub open spec fn issued_constructor_relation_v1(context: u64, allocations: usize, writers: usize, reads: usize,
    storage: StorageCapacitiesV1, result: Result<ProducerReadContentsV1, ReadErrorV1>) -> bool
{
    producer_constructor_relation_v1(context, allocations, writers, reads, result)
        && match result { Ok(contents) => issued_producer_v1(contents, storage, Seq::empty()), Err(_) => true }
}

pub fn issued_producer_constructor_exec_v1(context: u64, allocations: usize, writers: usize, reads: usize,
    Ghost(storage): Ghost<StorageCapacitiesV1>) -> (result: Result<ProducerReadContentsV1, ReadErrorV1>)
    requires storage_admission_v1(allocations, writers, storage),
    ensures issued_constructor_relation_v1(context, allocations, writers, reads, storage, result),
{
    let result = producer_constructor_exec_v1(context, allocations, writers, reads);
    let ghost snapshot = result;
    proof {
        if let Ok(contents) = snapshot {
            issued_constructor_v1(contents.stable.journal, context, allocations, writers, storage);
        }
    }
    result
}

pub open spec fn retaining_entry_v1(entry: Option<WriterEntryV1>) -> bool {
    match entry { Some(WriterEntryV1::Pending { .. }) | Some(WriterEntryV1::Unknown { .. }) => true, _ => false }
}

// Registration and Reserved abort may change only non-retaining writer entries.
pub open spec fn retaining_writers_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.writers@.len() == before.writers@.len()
    &&& forall|w: int| 0 <= w < before.writers@.len()
        && (retaining_entry_v1(#[trigger] before.writers@[w]) || retaining_entry_v1(#[trigger] after.writers@[w]))
        ==> before.writers@[w] == after.writers@[w]
}

pub proof fn retained_chain_issuance_frame_v1(before: JournalContentsV1, after: JournalContentsV1,
    writer: WriterReferenceV1, chain: Seq<usize>)
    requires issuance_contents_frame_v1(before, after), retaining_writers_frame_v1(before, after),
        retained_chain_v1(before, writer, chain),
    ensures retained_chain_v1(after, writer, chain),
{
    assert(retaining_entry_v1(before.writers@[writer.slot as int]));
    assert(after.writers@[writer.slot as int] == before.writers@[writer.slot as int]);
    assert forall|i: int| 0 <= i < chain.len() implies #[trigger] chain_link_v1(after, writer, chain, i) by {
        assert(chain_link_v1(before, writer, chain, i));
        reveal(chain_link_v1);
    }
}

pub proof fn custody_issuance_frame_v1(before: JournalContentsV1, after: JournalContentsV1,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires pending_custody_v1(before), issuance_contents_frame_v1(before, after),
        retaining_writers_frame_v1(before, after),
        issuance_invariant_v1(issuance_contents_projection_v1(after, storage, history)),
    ensures issued_custody_v1(after, storage, history),
{
    let post = issuance_contents_projection_v1(after, storage, history);
    issued_writer_partition_v1(post);
    assert forall|a: int| 0 <= a < after.allocations@.len() implies #[trigger] allocation_custody_v1(after, a) by {
        assert(allocation_custody_v1(before, a));
        reveal(allocation_custody_v1);
    }
    assert forall|m: int| 0 <= m < after.members@.len() implies #[trigger] member_custody_v1(after, m) by {
        assert(member_custody_v1(before, m));
        reveal(member_custody_v1);
        if before.members@[m].is_some() {
            let writer = before.members@[m].unwrap().writer;
            assert(retaining_entry_v1(before.writers@[writer.slot as int]));
            assert(after.writers@[writer.slot as int] == before.writers@[writer.slot as int]);
        }
    }
    assert forall|w: int| 0 <= w < after.writers@.len() implies #[trigger] writer_custody_v1(after, w) by {
        if after.writers@[w].is_some() {
            issued_writer_identity_v1(post, w);
            let entry = after.writers@[w].unwrap();
            reveal(writer_custody_v1);
            if retaining_entry_v1(after.writers@[w]) {
                assert(before.writers@[w] == after.writers@[w]);
                assert(writer_custody_v1(before, w));
                let writer = WriterReferenceV1 { slot: w as usize, key: writer_key_v1(entry) };
                let chain = choose|chain: Seq<usize>| #[trigger] retained_chain_v1(before, writer, chain);
                retained_chain_issuance_frame_v1(before, after, writer, chain);
            }
        } else { reveal(writer_custody_v1); }
    }
    assert forall|w: int, v: int| 0 <= w < v < after.writers@.len()
        && (#[trigger] after.writers@[w]).is_some() && (#[trigger] after.writers@[v]).is_some()
        implies writer_key_v1(after.writers@[w].unwrap()).local != writer_key_v1(after.writers@[v].unwrap()).local by {
        issued_writer_unique_v1(post, w, v);
    }
}

pub proof fn producer_status_issuance_frame_v1(before: JournalContentsV1, after: JournalContentsV1, request: ProducerReadV1)
    requires issuance_contents_frame_v1(before, after), retaining_writers_frame_v1(before, after),
    ensures producer_status_v1(before, request) == producer_status_v1(after, request),
{
    let slot = request.producer.slot;
    if slot < before.writers@.len() {
        if retaining_entry_v1(before.writers@[slot as int]) || retaining_entry_v1(after.writers@[slot as int]) {
            assert(before.writers@[slot as int] == after.writers@[slot as int]);
        }
    }
}

pub proof fn producer_issuance_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires producer_invariant_v1(before), producer_storage_frame_v1(before, after),
        issuance_contents_frame_v1(before.stable.journal, after.stable.journal),
        retaining_writers_frame_v1(before.stable.journal, after.stable.journal),
        issuance_invariant_v1(issuance_contents_projection_v1(after.stable.journal, storage, history)),
    ensures issued_producer_v1(after, storage, history),
{
    custody_issuance_frame_v1(before.stable.journal, after.stable.journal, storage, history);
    reader_allocation_frame_preserves_v1(before.stable, after.stable);
    assert forall|s: int| 0 <= s < after.reservations@.len() && (#[trigger] after.reservations@[s]).is_some()
        implies producer_entry_valid_v1(after.stable.journal, after.reservations@[s].unwrap(), s, after.next_incarnation) by {
        let entry = before.reservations@[s].unwrap();
        assert(producer_entry_valid_v1(before.stable.journal, entry, s, before.next_incarnation));
        reveal(producer_entry_valid_v1);
        producer_status_issuance_frame_v1(before.stable.journal, after.stable.journal, entry.request);
    }
}

pub proof fn register_issued_custody_v1(before: JournalContentsV1, after: JournalContentsV1, key: WriterKeyV1,
    result: Result<WriterReferenceV1, JournalErrorV1>, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_custody_v1(before, storage, history), issuance_contents_frame_v1(before, after),
        register_execution_relation_v1(before.context_generation, before.writer_capacity, key,
            before.writers@, before.free@, before.registration_watermark, before.reserved_count,
            after.writers@, after.free@, after.registration_watermark, after.reserved_count, result),
    ensures issued_custody_v1(after, storage, registration_history_v1(history, result)),
        retaining_writers_frame_v1(before, after),
{
    register_contents_refines_v1(before, after, key, result, storage, history);
    assert(retaining_writers_frame_v1(before, after));
    custody_issuance_frame_v1(before, after, storage, registration_history_v1(history, result));
}

pub proof fn abort_issued_custody_v1(before: JournalContentsV1, after: JournalContentsV1, reference: WriterReferenceV1,
    result: Result<(), JournalErrorV1>, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_custody_v1(before, storage, history), issuance_contents_frame_v1(before, after),
        abort_execution_relation_v1(before.context_generation, before.writer_capacity, storage.free, reference,
            before.writers@, before.free@, before.registration_watermark, before.reserved_count,
            after.writers@, after.free@, after.registration_watermark, after.reserved_count, result),
    ensures issued_custody_v1(after, storage, history), retaining_writers_frame_v1(before, after),
{
    abort_contents_refines_v1(before, after, reference, result, storage, history);
    assert(retaining_writers_frame_v1(before, after));
    custody_issuance_frame_v1(before, after, storage, history);
}

pub open spec fn issued_register_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    key: WriterKeyV1, result: Result<WriterReferenceV1, JournalErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>) -> bool
{
    &&& issued_producer_v1(after, storage, registration_history_v1(history, result))
    &&& producer_storage_frame_v1(before, after)
    &&& issuance_contents_frame_v1(before.stable.journal, after.stable.journal)
    &&& retaining_writers_frame_v1(before.stable.journal, after.stable.journal)
    &&& register_execution_relation_v1(before.stable.journal.context_generation, before.stable.journal.writer_capacity, key,
        before.stable.journal.writers@, before.stable.journal.free@,
        before.stable.journal.registration_watermark, before.stable.journal.reserved_count,
        after.stable.journal.writers@, after.stable.journal.free@,
        after.stable.journal.registration_watermark, after.stable.journal.reserved_count, result)
}

pub fn register_issued_producer_exec_v1(contents: &mut ProducerReadContentsV1, key: WriterKeyV1,
    Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<WriterReferenceV1, JournalErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history),
    ensures issued_register_relation_v1(*old(contents), *final(contents), key, result, storage, history),
{
    let ghost before = *contents;
    let result = register_reader_journal_v1(&mut contents.stable, key);
    proof {
        register_issued_custody_v1(before.stable.journal, contents.stable.journal, key, result, storage, history);
        producer_issuance_frame_v1(before, *contents, storage, registration_history_v1(history, result));
    }
    result
}

pub open spec fn issued_abort_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    observed_free_capacity: usize, reference: WriterReferenceV1, result: Result<(), JournalErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>) -> bool
{
    &&& issued_producer_v1(after, storage, history)
    &&& producer_storage_frame_v1(before, after)
    &&& issuance_contents_frame_v1(before.stable.journal, after.stable.journal)
    &&& retaining_writers_frame_v1(before.stable.journal, after.stable.journal)
    &&& abort_execution_relation_v1(before.stable.journal.context_generation, before.stable.journal.writer_capacity,
        observed_free_capacity, reference, before.stable.journal.writers@, before.stable.journal.free@,
        before.stable.journal.registration_watermark, before.stable.journal.reserved_count,
        after.stable.journal.writers@, after.stable.journal.free@,
        after.stable.journal.registration_watermark, after.stable.journal.reserved_count, result)
}

pub fn abort_issued_producer_exec_v1(contents: &mut ProducerReadContentsV1, observed_free_capacity: usize,
    reference: WriterReferenceV1, Ghost(storage): Ghost<StorageCapacitiesV1>, Ghost(history): Ghost<Seq<WriterReferenceV1>>)
    -> (result: Result<(), JournalErrorV1>)
    requires issued_producer_v1(*old(contents), storage, history), observed_free_capacity == storage.free,
    ensures issued_abort_relation_v1(*old(contents), *final(contents), observed_free_capacity, reference, result, storage, history),
{
    let ghost before = *contents;
    let result = abort_reader_journal_v1(&mut contents.stable, observed_free_capacity, reference);
    proof {
        abort_issued_custody_v1(before.stable.journal, contents.stable.journal, reference, result, storage, history);
        producer_issuance_frame_v1(before, *contents, storage, history);
    }
    contents.stable.journal.members.clear();
    result
}

pub fn issued_producer_reuse_witness_v1() -> (result: bool)
    ensures result,
{
    let ghost storage = witness_storage_v1(1, 1);
    let ghost empty = Seq::<WriterReferenceV1>::empty();
    let mut contents = match issued_producer_constructor_exec_v1(7, 1, 1, 2, Ghost(storage)) {
        Ok(contents) => contents, Err(_) => return false,
    };
    let first_key = WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission };
    let first = match register_issued_producer_exec_v1(&mut contents, first_key, Ghost(storage), Ghost(empty)) {
        Ok(reference) => reference, Err(_) => return false,
    };
    let ghost first_history = seq![first];
    assert(first.slot == 0 && contents.stable.journal.reserved_count == 1);
    let replay = register_issued_producer_exec_v1(&mut contents, first_key, Ghost(storage), Ghost(first_history));
    assert(replay == Err(JournalErrorV1::WriterReplay));
    let full_key = WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission };
    let full = register_issued_producer_exec_v1(&mut contents, full_key, Ghost(storage), Ghost(first_history));
    assert(full == Err(JournalErrorV1::WriterCapacity));
    assert(contents.stable.journal.registration_watermark == 10);
    let released = abort_issued_producer_exec_v1(&mut contents, 1, first, Ghost(storage), Ghost(first_history));
    assert(released == Ok(()));
    assert(contents.stable.journal.reserved_count == 0);
    let second_key = WriterKeyV1 { context_generation: 7, local: 12, kind: WriterKindV1::Synchronous };
    let second = match register_issued_producer_exec_v1(&mut contents, second_key, Ghost(storage), Ghost(first_history)) {
        Ok(reference) => reference, Err(_) => return false,
    };
    let ghost second_history = seq![first, second];
    assert(second.slot == first.slot);
    let stale = abort_issued_producer_exec_v1(&mut contents, 1, first, Ghost(storage), Ghost(second_history));
    assert(stale == Err(JournalErrorV1::InvalidReference));
    assert(contents.stable.journal.registration_watermark == 12);
    assert(contents.stable.journal.reserved_count == 1);
    assert(issued_producer_v1(contents, storage, second_history));
    true
}

pub open spec fn issued_fixture_entry_v1(index: usize) -> (AllocationEntryV1, ProducerReservationV1) {
    let key = AllocationKeyV1 { context_generation: 7, local: (index + 1) as u64 };
    let device = DeviceKeyV1 { context_generation: 7, local: 2 };
    let producer = WriterReferenceV1 { slot: index,
        key: WriterKeyV1 { context_generation: 7, local: (10 + index) as u64, kind: WriterKindV1::Submission } };
    let entry = AllocationEntryV1 { key, device, byte_extent: 16, attempt_epoch: 1,
        content_lineage: if index == 2 { 1 } else { 0 }, pending_member: if index < 2 { Some(index) } else { None } };
    let reservation = ProducerReservationV1 {
        request: ProducerReadV1 { producer, read: AllocationReadV1 {
            allocation: AllocationReferenceV1 { slot: index, key }, device, byte_extent: 16,
            byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0,
        } },
        reference: ProducerReadReferenceV1 { slot: index, incarnation: (index + 1) as u64,
            consumer: WriterKeyV1 { context_generation: 7, local: (20 + index) as u64, kind: WriterKindV1::Submission } },
    };
    (entry, reservation)
}

pub fn issued_fixture_entry_exec_v1(index: usize) -> (result: (AllocationEntryV1, ProducerReservationV1))
    requires index < 4,
    ensures result == issued_fixture_entry_v1(index),
{
    let key = AllocationKeyV1 { context_generation: 7, local: (index + 1) as u64 };
    let device = DeviceKeyV1 { context_generation: 7, local: 2 };
    let producer = WriterReferenceV1 { slot: index,
        key: WriterKeyV1 { context_generation: 7, local: (10 + index) as u64, kind: WriterKindV1::Submission } };
    let entry = AllocationEntryV1 { key, device, byte_extent: 16, attempt_epoch: 1,
        content_lineage: if index == 2 { 1 } else { 0 }, pending_member: if index < 2 { Some(index) } else { None } };
    let reservation = ProducerReservationV1 {
        request: ProducerReadV1 { producer, read: AllocationReadV1 {
            allocation: AllocationReferenceV1 { slot: index, key }, device, byte_extent: 16,
            byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0,
        } },
        reference: ProducerReadReferenceV1 { slot: index, incarnation: (index + 1) as u64,
            consumer: WriterKeyV1 { context_generation: 7, local: (20 + index) as u64, kind: WriterKindV1::Submission } },
    };
    (entry, reservation)
}

pub open spec fn issued_fixture_history_v1() -> Seq<WriterReferenceV1> {
    seq![issued_fixture_entry_v1(0).1.request.producer, issued_fixture_entry_v1(1).1.request.producer,
        issued_fixture_entry_v1(2).1.request.producer, issued_fixture_entry_v1(3).1.request.producer]
}

pub open spec fn issued_fixture_member_v1(index: usize) -> MemberEntryV1 {
    let request = issued_fixture_entry_v1(index).1.request;
    MemberEntryV1 { writer: request.producer, allocation: request.read.allocation,
        prior_lineage: 0, attempt_epoch: 1, next: None }
}

pub open spec fn issued_fixture_journal_v1(journal: JournalContentsV1) -> bool {
    &&& journal.context_generation == 7
    &&& journal.allocation_capacity == 4
    &&& journal.writer_capacity == 4
    &&& journal.registration_watermark == 13
    &&& journal.reserved_count == 0
    &&& journal.allocations@ == Seq::new(4, |i: int| Some(issued_fixture_entry_v1(i as usize).0))
    &&& journal.allocation_free@ == Seq::<usize>::empty()
    &&& journal.members@ == seq![Some(issued_fixture_member_v1(0)), Some(issued_fixture_member_v1(1)), None, None]
    &&& journal.member_free@ == seq![3usize, 2usize]
    &&& journal.writers@ == seq![
        Some(WriterEntryV1::Pending { key: issued_fixture_entry_v1(0).1.request.producer.key, head: Some(0), count: 1 }),
        Some(WriterEntryV1::Unknown { key: issued_fixture_entry_v1(1).1.request.producer.key, head: Some(1), count: 1 }), None, None]
    &&& journal.free@ == seq![3usize, 2usize]
    &&& journal.scratch@ == Seq::new(4, |i: int| None)
}

#[verifier::spinoff_prover]
pub proof fn issued_fixture_custody_v1(journal: JournalContentsV1)
    requires issued_fixture_journal_v1(journal),
    ensures pending_custody_v1(journal),
{
    let p0 = issued_fixture_entry_v1(0).1.request.producer;
    let p1 = issued_fixture_entry_v1(1).1.request.producer;
    reveal(chain_link_v1);
    assert(chain_link_v1(journal, p0, seq![0usize], 0));
    assert(chain_link_v1(journal, p1, seq![1usize], 0));
    assert forall|m: int| 0 <= m < journal.members@.len() && (#[trigger] journal.members@[m]).is_some()
        && same_producer_v1(journal.members@[m].unwrap().writer, p0)
        implies seq![0usize].contains(m as usize) by { assert(m == 0); }
    assert forall|m: int| 0 <= m < journal.members@.len() && (#[trigger] journal.members@[m]).is_some()
        && same_producer_v1(journal.members@[m].unwrap().writer, p1)
        implies seq![1usize].contains(m as usize) by { assert(m == 1); }
    assert(retained_chain_v1(journal, p0, seq![0usize]));
    assert(retained_chain_v1(journal, p1, seq![1usize]));
    reveal(allocation_custody_v1);
    reveal(member_custody_v1);
    reveal(writer_custody_v1);
}

#[verifier::spinoff_prover]
pub proof fn issued_fixture_issuance_v1(journal: JournalContentsV1)
    requires issued_fixture_journal_v1(journal),
    ensures issuance_invariant_v1(issuance_contents_projection_v1(journal, witness_storage_v1(4, 4), issued_fixture_history_v1())),
{
    reveal_with_fuel(occupied_prefix_v1, 5);
    reveal_with_fuel(reserved_prefix_v1, 5);
}

#[verifier::spinoff_prover]
pub proof fn issued_fixture_producer_v1(contents: ProducerReadContentsV1)
    requires issued_fixture_journal_v1(contents.stable.journal), reader_invariant_v1(contents.stable),
        contents.stable.leases@.len() == 4, contents.stable.free_reads@.len() == 4,
        contents.reservations@ == Seq::new(4, |i: int| Some(issued_fixture_entry_v1(i as usize).1)),
        contents.free@ == Seq::<usize>::empty(), contents.counts@ == seq![1usize, 1usize, 1usize, 1usize],
        contents.next_incarnation == 5,
    ensures issued_producer_v1(contents, witness_storage_v1(4, 4), issued_fixture_history_v1()),
{
    issued_fixture_custody_v1(contents.stable.journal);
    issued_fixture_issuance_v1(contents.stable.journal);
    reveal(producer_entry_valid_v1);
    reveal_with_fuel(producer_count_v1, 5);
}

// Direct fixture initialization tests mixed-state nonvacuity, not begin-write reachability.
#[verifier::spinoff_prover]
pub fn issued_producer_mixed_fixture_v1() -> (result: Option<ProducerReadContentsV1>)
    ensures result.is_some(), issued_producer_v1(result.unwrap(), witness_storage_v1(4, 4), issued_fixture_history_v1()),
        result.unwrap().stable.journal.context_generation == 7,
        result.unwrap().stable.journal.writer_capacity == 4,
        result.unwrap().stable.journal.registration_watermark == 13,
        result.unwrap().stable.journal.reserved_count == 0,
        result.unwrap().stable.journal.free@ == seq![3usize, 2usize],
        result.unwrap().counts@ == seq![1usize, 1usize, 1usize, 1usize],
        producer_status_v1(result.unwrap().stable.journal, issued_fixture_entry_v1(0).1.request) == Some(ProducerStatusV1::Pending),
        producer_status_v1(result.unwrap().stable.journal, issued_fixture_entry_v1(1).1.request) == Some(ProducerStatusV1::Unknown),
        producer_status_v1(result.unwrap().stable.journal, issued_fixture_entry_v1(2).1.request) == Some(ProducerStatusV1::Success),
        producer_status_v1(result.unwrap().stable.journal, issued_fixture_entry_v1(3).1.request) == Some(ProducerStatusV1::NoEffect),
{
    let ghost storage = witness_storage_v1(4, 4);
    let mut contents = match issued_producer_constructor_exec_v1(7, 4, 4, 4, Ghost(storage)) {
        Ok(contents) => contents, Err(_) => return None,
    };
    let ghost empty = contents.stable;
    let (a0, r0) = issued_fixture_entry_exec_v1(0);
    let (a1, r1) = issued_fixture_entry_exec_v1(1);
    let (a2, r2) = issued_fixture_entry_exec_v1(2);
    let (a3, r3) = issued_fixture_entry_exec_v1(3);
    let p0 = r0.request.producer;
    let p1 = r1.request.producer;
    contents.stable.journal.allocations.set(0, Some(a0));
    contents.stable.journal.allocations.set(1, Some(a1));
    contents.stable.journal.allocations.set(2, Some(a2));
    contents.stable.journal.allocations.set(3, Some(a3));
    contents.stable.journal.allocation_free.clear();
    contents.stable.journal.members.set(0, Some(MemberEntryV1 { writer: p0, allocation: r0.request.read.allocation,
        prior_lineage: 0, attempt_epoch: 1, next: None }));
    contents.stable.journal.members.set(1, Some(MemberEntryV1 { writer: p1, allocation: r1.request.read.allocation,
        prior_lineage: 0, attempt_epoch: 1, next: None }));
    contents.stable.journal.member_free.truncate(2);
    contents.stable.journal.writers.set(0, Some(WriterEntryV1::Pending { key: p0.key, head: Some(0), count: 1 }));
    contents.stable.journal.writers.set(1, Some(WriterEntryV1::Unknown { key: p1.key, head: Some(1), count: 1 }));
    contents.stable.journal.free.truncate(2);
    contents.stable.journal.registration_watermark = 13;
    contents.reservations.set(0, Some(r0));
    contents.reservations.set(1, Some(r1));
    contents.reservations.set(2, Some(r2));
    contents.reservations.set(3, Some(r3));
    contents.free.clear();
    contents.counts.set(0, 1);
    contents.counts.set(1, 1);
    contents.counts.set(2, 1);
    contents.counts.set(3, 1);
    contents.next_incarnation = 5;
    proof {
        reader_allocation_frame_preserves_v1(empty, contents.stable);
        assert(contents.stable.journal.allocations@ =~= Seq::new(4, |i: int| Some(issued_fixture_entry_v1(i as usize).0)));
        assert(contents.reservations@ =~= Seq::new(4, |i: int| Some(issued_fixture_entry_v1(i as usize).1)));
        assert(contents.stable.journal.members@ =~= seq![Some(issued_fixture_member_v1(0)), Some(issued_fixture_member_v1(1)), None, None]);
        assert(contents.stable.journal.writers@ =~= seq![
            Some(WriterEntryV1::Pending { key: p0.key, head: Some(0), count: 1 }),
            Some(WriterEntryV1::Unknown { key: p1.key, head: Some(1), count: 1 }), None, None]);
        assert(contents.stable.journal.member_free@ =~= seq![3usize, 2usize]);
        assert(contents.stable.journal.free@ =~= seq![3usize, 2usize]);
        assert(contents.counts@ =~= seq![1usize, 1usize, 1usize, 1usize]);
        assert(issued_fixture_journal_v1(contents.stable.journal));
        issued_fixture_producer_v1(contents);
    }
    Some(contents)
}

#[verifier::spinoff_prover]
pub fn issued_producer_mixed_witness_v1() -> (result: bool)
    ensures result,
{
    let ghost storage = witness_storage_v1(4, 4);
    let ghost history = issued_fixture_history_v1();
    let mut contents = match issued_producer_mixed_fixture_v1() { Some(contents) => contents, None => return false };
    let (_a0, r0) = issued_fixture_entry_exec_v1(0);
    let (_a1, r1) = issued_fixture_entry_exec_v1(1);
    let (_a2, r2) = issued_fixture_entry_exec_v1(2);
    let (_a3, r3) = issued_fixture_entry_exec_v1(3);
    let ghost before = contents;
    let key = WriterKeyV1 { context_generation: 7, local: 30, kind: WriterKindV1::Submission };
    assert(contents.stable.journal.free@.contains(2usize));
    assert(contents.stable.journal.writers@[2].is_none());
    let reference = match register_issued_producer_exec_v1(&mut contents, key, Ghost(storage), Ghost(history)) {
        Ok(reference) => reference, Err(_) => return false,
    };
    assert(reference.slot == r2.request.producer.slot);
    let ghost registered = contents;
    let ghost registered_history = history.push(reference);
    let aborted = abort_issued_producer_exec_v1(&mut contents, 4, reference, Ghost(storage), Ghost(registered_history));
    assert(aborted == Ok(()));
    proof {
        producer_status_issuance_frame_v1(before.stable.journal, registered.stable.journal, r0.request);
        producer_status_issuance_frame_v1(before.stable.journal, registered.stable.journal, r1.request);
        producer_status_issuance_frame_v1(before.stable.journal, registered.stable.journal, r2.request);
        producer_status_issuance_frame_v1(before.stable.journal, registered.stable.journal, r3.request);
        producer_status_issuance_frame_v1(registered.stable.journal, contents.stable.journal, r0.request);
        producer_status_issuance_frame_v1(registered.stable.journal, contents.stable.journal, r1.request);
        producer_status_issuance_frame_v1(registered.stable.journal, contents.stable.journal, r2.request);
        producer_status_issuance_frame_v1(registered.stable.journal, contents.stable.journal, r3.request);
        assert(producer_status_v1(contents.stable.journal, r0.request) == Some(ProducerStatusV1::Pending));
        assert(producer_status_v1(contents.stable.journal, r1.request) == Some(ProducerStatusV1::Unknown));
        assert(producer_status_v1(contents.stable.journal, r2.request) == Some(ProducerStatusV1::Success));
        assert(producer_status_v1(contents.stable.journal, r3.request) == Some(ProducerStatusV1::NoEffect));
        assert(contents.stable.journal.members@ == before.stable.journal.members@);
        assert(contents.stable.journal.writers@[0] == before.stable.journal.writers@[0]);
        assert(contents.stable.journal.writers@[1] == before.stable.journal.writers@[1]);
        assert(contents.counts@ == seq![1usize, 1usize, 1usize, 1usize]);
        assert(issued_producer_v1(contents, storage, registered_history));
    }
    true
}

}
}

pub use journal_issuance::*;

// Exact stable-wrapper declarations from the historical lifecycle, in the importing type universe.
use super::*;

verus! {

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

}

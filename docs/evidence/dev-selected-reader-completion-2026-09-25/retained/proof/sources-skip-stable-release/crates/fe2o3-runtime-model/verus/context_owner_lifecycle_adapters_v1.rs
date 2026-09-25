// Constructor-origin composition adapters over raw transitions, not post-invariant assumptions.
use super::*;

verus! {

pub open spec fn lifecycle_begin_relation_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>) -> bool
{
    &&& result == begin_issued_decision_v1(before, writer, roster)
    &&& producer_storage_frame_v1(before, after)
    &&& result.is_err() ==> after == before
    &&& result.is_ok() ==> begin_success_relation_v1(before.stable.journal, after.stable.journal, writer, roster)
}

pub proof fn lifecycle_begin_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, roster: Seq<AllocationWriteV1>, result: Result<(), ReadErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history),
        lifecycle_begin_relation_v1(before, after, writer, roster, result),
    ensures issued_producer_v1(after, storage, history),
        begin_issued_relation_v1(before, after, writer, roster, result, storage, history),
{
    if let Ok(value) = result {
        assert(value == ());
        match producer_unread_scan_v1(before, begin_read_references_v1(roster), 0) {
            Ok(value) => { assert(value == ()); }, Err(_) => { assert(false); },
        }
        begin_unread_scan_selected_v1(before, roster);
        assert(begin_execution_relation_v1(before.stable.journal, after.stable.journal, writer, roster, result));
        begin_preserves_issued_producer_v1(before, after, writer, roster, result, storage, history);
    }
}

pub proof fn lifecycle_batch_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    entries: Seq<EnrollmentV1>, original: Seq<Option<AllocationReferenceV1>>,
    output: Seq<Option<AllocationReferenceV1>>, result: Result<(), EnrollmentErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history), producer_storage_frame_v1(before, after),
        enrollment_execution_relation_v1(before.stable.journal, after.stable.journal, entries, original, output, result),
    ensures issued_producer_v1(after, storage, history),
{
    if let Ok(value) = result {
        assert(value == ());
        enrollment_success_admission_v1(before.stable.journal, entries, original);
        let remaining = (before.stable.journal.allocation_free@.len() - entries.len()) as usize;
        assert(enrollment_final_relation_v1(before.stable, after.stable, entries, output, remaining));
        enrollment_preserves_issued_producer_v1(before, after, entries, output, remaining, storage, history);
    }
}

pub proof fn lifecycle_unknown_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    writer: WriterReferenceV1, result: Result<(), ReadErrorV1>, storage: StorageCapacitiesV1,
    history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history), producer_storage_frame_v1(before, after),
        unknown_execution_relation_v1(before.stable.journal, after.stable.journal, writer, result),
    ensures issued_producer_v1(after, storage, history),
{
    if result.is_ok() {
        unknown_preserves_producer_invariant_v1(before, after, writer);
        let pre = issuance_contents_projection_v1(before.stable.journal, storage, history);
        let post = issuance_contents_projection_v1(after.stable.journal, storage, history);
        prefix_update_v1(pre.writers, writer.slot as int, post.writers[writer.slot as int], pre.writers.len() as int);
        assert forall|w: int| #![trigger post.writers[w]] 0 <= w < post.writers.len() implies match post.writers[w] {
            Some(entry) => {
                &&& exists|i: int| 0 <= i < history.len() && history[i].slot == w
                    && same_key_v1(history[i].key, writer_key_v1(entry))
                &&& forall|i: int| 0 <= i < history.len() && history[i].slot == w
                    ==> history[i].key.local <= writer_key_v1(entry).local
            },
            None => true,
        } by { assert(pre.writers[w].is_some() == post.writers[w].is_some()); }
    }
}

pub proof fn lifecycle_read_issuance_frame_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history), producer_invariant_v1(after),
        reader_journal_frame_v1(before.stable, after.stable),
    ensures issued_producer_v1(after, storage, history),
{
}

pub proof fn lifecycle_abort_preserves_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    reference: WriterReferenceV1, observed_free_capacity: usize, result: Result<(), JournalErrorV1>,
    storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>)
    requires issued_producer_v1(before, storage, history), producer_storage_frame_v1(before, after),
        issuance_contents_frame_v1(before.stable.journal, after.stable.journal),
        abort_execution_relation_v1(before.stable.journal.context_generation, before.stable.journal.writer_capacity,
            observed_free_capacity, reference, before.stable.journal.writers@, before.stable.journal.free@,
            before.stable.journal.registration_watermark, before.stable.journal.reserved_count,
            after.stable.journal.writers@, after.stable.journal.free@,
            after.stable.journal.registration_watermark, after.stable.journal.reserved_count, result),
    ensures issued_producer_v1(after, storage, history),
{
    if result.is_ok() {
        let journal = before.stable.journal;
        assert(abort_decision_v1(journal.context_generation, journal.reserved_count, journal.writer_capacity,
            journal.writers@, journal.free@.len() as usize, storage.free, reference).is_ok());
        assert(abort_execution_relation_v1(journal.context_generation, journal.writer_capacity, storage.free, reference,
            journal.writers@, journal.free@, journal.registration_watermark, journal.reserved_count,
            after.stable.journal.writers@, after.stable.journal.free@,
            after.stable.journal.registration_watermark, after.stable.journal.reserved_count, result));
        abort_issued_custody_v1(journal, after.stable.journal, reference, result, storage, history);
    }
    producer_issuance_frame_v1(before, after, storage, history);
}

pub open spec fn lifecycle_producer_epoch_step_v1(before: u64, after: u64,
    minted: Seq<ProducerReadReferenceV1>) -> bool
{
    &&& 0 < before
    &&& after == before + minted.len()
    &&& forall|i: int| 0 <= i < minted.len() ==> (#[trigger] minted[i]).incarnation == before + i
}

pub proof fn lifecycle_producer_acquire_epoch_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<ProducerReadV1>, original: Seq<Option<ProducerReadReferenceV1>>,
    output: Seq<Option<ProducerReadReferenceV1>>, result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(before), requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        producer_acquire_execution_relation_v1(before, after, consumer, requests, original, output, result),
    ensures lifecycle_producer_epoch_step_v1(before.next_incarnation, after.next_incarnation,
        if result.is_ok() { Seq::new(output.len(), |i: int| output[i].unwrap()) } else { Seq::empty() }),
{
    if let Ok(value) = result {
        assert(value == ());
        producer_acquire_preflight_ready_v1(before, consumer, requests, original);
    }
}

pub proof fn lifecycle_stable_acquire_epoch_v1(before: ProducerReadContentsV1, after: ProducerReadContentsV1,
    consumer: WriterKeyV1, requests: Seq<AllocationReadV1>, original: Seq<Option<ReadReferenceV1>>,
    output: Seq<Option<ReadReferenceV1>>, result: Result<(), ReadErrorV1>)
    requires producer_invariant_v1(before), requests.len() <= usize::MAX, requests.len() <= u64::MAX,
        stable_wrapper_acquire_relation_v1(before, after, consumer, requests, original, output, result),
    ensures reader_epoch_step_v1(before.stable.next_incarnation, after.stable.next_incarnation,
        if result.is_ok() { Seq::new(output.len(), |i: int| output[i].unwrap()) } else { Seq::empty() }),
{
    if stable_wrapper_acquire_header_v1(before, consumer, requests.len() as usize, original).is_ok() {
        acquire_epoch_projection_v1(before.stable, after.stable, consumer, requests, original, output, result);
    }
}

}

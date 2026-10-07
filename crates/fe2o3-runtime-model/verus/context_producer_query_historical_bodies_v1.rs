// Exact query declarations from the historical lifecycle, in the importing root's type universe.
use super::*;

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

}

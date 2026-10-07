verus! {

spec fn writer_lookup_decision_v1(journal: JournalContentsV1, reference: WriterReferenceV1)
    -> Result<ContextWriterStateV1, ReadErrorV1>
{
    if reference.slot >= journal.writers@.len() { Err(ReadErrorV1::InvalidReference) }
    else { match journal.writers@[reference.slot as int] {
        None => Err(ReadErrorV1::InvalidReference),
        Some(entry) => {
            let (key, state) = match entry {
                WriterEntryV1::Reserved(key) => (key, ContextWriterStateV1::Reserved),
                WriterEntryV1::Pending { key, count, .. } => (key, ContextWriterStateV1::Pending { member_count: count }),
                WriterEntryV1::Unknown { key, count, .. } => (key, ContextWriterStateV1::Unknown { member_count: count }),
            };
            if key != reference.key || key.context_generation != journal.context_generation {
                Err(ReadErrorV1::InvalidReference)
            } else { Ok(state) }
        },
    } }
}

spec fn producer_status_decision_v1(journal: JournalContentsV1, request: ContextProducerReadV1)
    -> Result<ContextProducerReadStatusV1, ReadErrorV1>
{
    match allocation_lookup_decision_v1(journal, request.read.allocation) {
        Err(error) => Err(error),
        Ok(state) => {
            let read = request.read;
            if state.device != read.device { Err(ReadErrorV1::AllocationDeviceMismatch) }
            else if state.byte_extent != read.byte_extent { Err(ReadErrorV1::AllocationExtentMismatch) }
            else if read.byte_len == 0 || read.byte_offset + read.byte_len > u64::MAX
                || read.byte_offset + read.byte_len > read.byte_extent { Err(ReadErrorV1::InvalidExtent) }
            else if request.producer.key.context_generation != journal.context_generation
                || request.producer.key.kind != WriterKindV1::Submission
                || read.content_lineage >= read.attempt_epoch
                || state.attempt_epoch != read.attempt_epoch { Err(ReadErrorV1::InvalidState) }
            else { match state.pending_writer {
                Some(writer) => {
                    if writer != request.producer || state.content_lineage != read.content_lineage { Err(ReadErrorV1::InvalidState) }
                    else { match writer_lookup_decision_v1(journal, writer) {
                        Err(error) => Err(error),
                        Ok(ContextWriterStateV1::Reserved) => Err(ReadErrorV1::InvalidState),
                        Ok(ContextWriterStateV1::Pending { .. }) => Ok(ContextProducerReadStatusV1::Pending),
                        Ok(ContextWriterStateV1::Unknown { .. }) => Ok(ContextProducerReadStatusV1::Unknown),
                    } }
                },
                None => if state.content_lineage == read.attempt_epoch { Ok(ContextProducerReadStatusV1::Success) }
                    else if state.content_lineage == read.content_lineage { Ok(ContextProducerReadStatusV1::NoEffect) }
                    else { Err(ReadErrorV1::InvalidState) },
            } }
        },
    }
}

spec fn producer_validate_decision_v1(contents: ContextProducerReadJournalV1, request: ContextProducerReadV1)
    -> Result<(), ReadErrorV1>
{
    match producer_status_decision_v1(contents.stable.journal, request) {
        Err(error) => Err(error),
        Ok(ContextProducerReadStatusV1::Pending) => Ok(()),
        Ok(_) => Err(ReadErrorV1::AllocationBusy),
    }
}

spec fn producer_inspect_decision_v1(contents: ContextProducerReadJournalV1, reference: ContextProducerReadReferenceV1)
    -> Result<(ContextProducerReadV1, ContextProducerReadStatusV1), ReadErrorV1>
{
    if reference.slot >= contents.reservations@.len() { Err(ReadErrorV1::InvalidReference) }
    else { match contents.reservations@[reference.slot as int] {
        None => Err(ReadErrorV1::InvalidReference),
        Some(entry) => if entry.reference != reference { Err(ReadErrorV1::InvalidReference) }
            else { match producer_status_decision_v1(contents.stable.journal, entry.request) {
                Err(error) => Err(error), Ok(status) => Ok((entry.request, status)),
            } },
    } }
}

spec fn producer_lookup_decision_v1(contents: ContextProducerReadJournalV1, reference: ContextProducerReadReferenceV1)
    -> Result<ContextProducerReadV1, ReadErrorV1>
{
    match producer_inspect_decision_v1(contents, reference) {
        Err(error) => Err(error), Ok((request, _)) => Ok(request),
    }
}

spec fn producer_query_decision_v1(contents: ContextProducerReadJournalV1, reference: ContextProducerReadReferenceV1)
    -> Result<ContextProducerReadStatusV1, ReadErrorV1>
{
    match producer_inspect_decision_v1(contents, reference) {
        Err(error) => Err(error), Ok((_, status)) => Ok(status),
    }
}

}

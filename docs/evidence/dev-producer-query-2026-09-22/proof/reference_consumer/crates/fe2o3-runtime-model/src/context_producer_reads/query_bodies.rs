macro_rules! producer_writer_same_body {
    ($left:ident, $right:ident) => {
        $left.slot == $right.slot && producer_query_consumer_same_exec_v1($left.key, $right.key)
    };
}

macro_rules! producer_reference_same_body {
    ($left:ident, $right:ident) => {
        $left.slot == $right.slot && $left.incarnation == $right.incarnation
            && true
    };
}

macro_rules! producer_status_body {
    ($journal:expr, $request:ident, $same:path) => {{
        let journal: &ContextVersionJournalV1 = $journal;
        let read = $request.read;
        let state = match journal.lookup_allocation(read.allocation) {
            Ok(state) => state,
            Err(error) => return Err(error),
        };
        if state.device.context_generation != read.device.context_generation
            || state.device.local != read.device.local
        {
            return Err(ContextVersionJournalErrorV1::AllocationDeviceMismatch);
        }
        if state.byte_extent != read.byte_extent {
            return Err(ContextVersionJournalErrorV1::AllocationExtentMismatch);
        }
        if read.byte_len == 0 {
            return Err(ContextVersionJournalErrorV1::InvalidExtent);
        }
        let end = match read.byte_offset.checked_add(read.byte_len) {
            Some(end) => end,
            None => return Err(ContextVersionJournalErrorV1::InvalidExtent),
        };
        if end > read.byte_extent {
            return Err(ContextVersionJournalErrorV1::InvalidExtent);
        }
        if $request.producer.key.context_generation != journal.context_generation()
            || !matches!($request.producer.key.kind, ContextWriterKindV1::Submission)
            || read.content_lineage >= read.attempt_epoch
            || state.attempt_epoch != read.attempt_epoch
        {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        match state.pending_writer {
            Some(writer) => {
                if !$same(writer, $request.producer) || state.content_lineage != read.content_lineage {
                    return Err(ContextVersionJournalErrorV1::InvalidState);
                }
                let state = match journal.lookup_writer(writer) {
                    Ok(state) => state,
                    Err(error) => return Err(error),
                };
                match state {
                    ContextWriterStateV1::Pending { .. } => Ok(ContextProducerReadStatusV1::Pending),
                    ContextWriterStateV1::Unknown { .. } => Ok(ContextProducerReadStatusV1::Unknown),
                    ContextWriterStateV1::Reserved => Err(ContextVersionJournalErrorV1::InvalidState),
                }
            }
            // Resolved data does not depend on the old producer slot's current contents.
            None if state.content_lineage == read.attempt_epoch => Ok(ContextProducerReadStatusV1::Success),
            None if state.content_lineage == read.content_lineage => Ok(ContextProducerReadStatusV1::NoEffect),
            None => Err(ContextVersionJournalErrorV1::InvalidState),
        }
    }};
}

macro_rules! producer_validate_body {
    ($contents:ident, $request:ident) => {{
        match $contents.status($request) {
            Err(error) => Err(error),
            Ok(ContextProducerReadStatusV1::Pending) => Ok(()),
            Ok(_) => Err(ContextVersionJournalErrorV1::AllocationBusy),
        }
    }};
}

macro_rules! producer_inspect_body {
    ($contents:ident, $reference:ident, $same:path) => {{
        if $reference.slot >= $contents.reservations.len() {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        let entry = match $contents.reservations[$reference.slot] {
            Some(entry) => entry,
            None => return Err(ContextVersionJournalErrorV1::InvalidReference),
        };
        if !$same(entry.reference, $reference) {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        match $contents.status(&entry.request) {
            Err(error) => Err(error),
            Ok(status) => Ok((entry.request, status)),
        }
    }};
}

macro_rules! producer_lookup_body {
    ($contents:ident, $reference:ident) => {{
        match $contents.inspect_producer_read($reference) {
            Err(error) => Err(error),
            Ok((request, _)) => Ok(request),
        }
    }};
}

macro_rules! producer_query_body {
    ($contents:ident, $reference:ident) => {{
        match $contents.inspect_producer_read($reference) {
            Err(error) => Err(error),
            Ok((_, status)) => Ok(status),
        }
    }};
}

// Shared constant-work writer lifecycle. Capacity remains a lazy observation.
macro_rules! writer_register_body {
    ($syntax:ident, $journal:ident, $key:ident, $issuable:path, $count:path) => {
        $syntax!({
            if $key.context_generation != $journal.context_generation {
                return Err(ContextVersionJournalErrorV1::ForeignContext);
            }
            if !$issuable($key.local) { return Err(ContextVersionJournalErrorV1::InvalidWriterId); }
            if $key.local <= $journal.registration_watermark {
                return Err(ContextVersionJournalErrorV1::WriterReplay);
            }
            $count($journal);
            if $journal.free.len() == 0 { return Err(ContextVersionJournalErrorV1::WriterCapacity); }
            let slot = $journal.free[$journal.free.len() - 1];
            $count($journal);
            if slot >= $journal.writers.len() || $journal.writers[slot].is_some() {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            let reserved_count = match $journal.reserved_count.checked_add(1) {
                Some(count) => count,
                None => return Err(ContextVersionJournalErrorV1::InvalidState),
            };
            if reserved_count > $journal.writer_capacity { return Err(ContextVersionJournalErrorV1::InvalidState); }
            $count($journal);
            let _ = $journal.free.pop();
            $count($journal);
            $journal.writers[slot] = Some(WriterEntryV1::Reserved($key));
            $journal.reserved_count = reserved_count;
            $journal.registration_watermark = $key.local;
            Ok(ContextWriterReferenceV1 { slot, key: $key })
        })
    };
}

macro_rules! writer_reserved_lookup_body {
    ($journal:ident, $reference:ident, $lookup:path) => {
        $lookup($journal, $reference)
    };
}

macro_rules! writer_abort_body {
    ($syntax:ident, $journal:ident, $reference:ident, $capacity:expr, $count:path) => {
        $syntax!({
            if let Err(error) = $journal.lookup_reserved($reference) { return Err(error); }
            if $journal.free.len() >= $journal.writer_capacity || $journal.free.len() >= $capacity {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            let reserved_count = match $journal.reserved_count.checked_sub(1) {
                Some(count) => count,
                None => return Err(ContextVersionJournalErrorV1::InvalidState),
            };
            $count($journal);
            $journal.writers[$reference.slot] = None;
            $count($journal);
            $journal.free.push($reference.slot);
            $journal.reserved_count = reserved_count;
            Ok(())
        })
    };
}

macro_rules! writer_owner_forward_body {
    ($owner:ident, $field:ident, $method:ident, $argument:ident, [$($extra:tt)*]) => {
        $owner.$field.$method($argument $($extra)*)
    };
}

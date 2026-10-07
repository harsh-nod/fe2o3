// Capacity expressions are evaluated once, after the complete retained chain.
macro_rules! settlement_preflight_body {
    ($syntax:ident, $journal:ident, $writer:ident, $evidence:ident,
     $writer_capacity:expr, $member_capacity:expr,
     $header:path, $same_key:path, $chain:path, $scan:path) => {
        $syntax!({
            let (head, count, _) = match $header($journal, $writer, false) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            if $evidence.slot != $writer.slot || !$same_key($evidence.key, $writer.key) {
                return Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch);
            }
            if let Err(error) = $chain($journal, $writer, head, count) { return Err(error); }
            let storage = SettlementReturnStorageV1 {
                writer_free_len: $journal.free.len(),
                member_free_len: $journal.member_free.len(),
                writer_limit: $journal.writer_capacity,
                writer_storage: $writer_capacity,
                member_limit: $journal.allocation_capacity,
                member_storage: $member_capacity,
                scratch_len: $journal.scratch.len(),
            };
            if let Err(error) = storage.check(count) { return Err(error); }
            if let Err(error) = $scan($journal, count) { return Err(error); }
            Ok((head, count))
        })
    };
}

macro_rules! settlement_execute_body {
    ($syntax:ident, $journal:ident, $writer:ident, $evidence:ident, $success:ident,
     $head:ident, $count:ident, $preflight:ident, [$($observations:tt)*],
     $stage:path, $commit:path, [$($commit_extra:tt)*], [$($before:tt)*], [$($ready:tt)*]) => {
        $syntax!({
            $($before)*
            let ($head, $count) = match $journal.$preflight($writer, $evidence $($observations)*) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            $($ready)*
            $stage($journal, $head, $count);
            $commit($journal, $writer, $count, $success $($commit_extra)*);
            Ok(())
        })
    };
}

macro_rules! settlement_outcome_body {
    ($journal:ident, $method:ident, $writer:ident, $evidence:ident, $success:literal, [$($extra:tt)*]) => {
        $journal.$method($writer, $evidence.writer, $success $($extra)*)
    };
}

macro_rules! settlement_owner_forward_body {
    ($owner:ident, $field:ident, $method:ident, $writer:ident, $evidence:ident, [$($extra:tt)*]) => {
        $owner.$field.$method($writer, $evidence $($extra)*)
    };
}

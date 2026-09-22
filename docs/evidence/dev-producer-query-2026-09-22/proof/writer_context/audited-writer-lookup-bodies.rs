 writer_lookup_body {
    ($contents:ident, $reference:ident, $same:path, $count:path) => {{
        $count($contents);
        if $reference.slot >= $contents.writers.len() {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        let (key, state) = match $contents.writers[$reference.slot] {
            Some(WriterEntryV1::Reserved(key)) => (key, ContextWriterStateV1::Reserved),
            Some(WriterEntryV1::Pending { key, count, .. }) => (
                key,
                ContextWriterStateV1::Pending { member_count: count },
            ),
            Some(WriterEntryV1::Unknown { key, count, .. }) => (
                key,
                ContextWriterStateV1::Unknown { member_count: count },
            ),
            None => return Err(ContextVersionJournalErrorV1::InvalidReference),
        };
        if !$same(key, $reference.key) || false {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        Ok(state)
    }};
}

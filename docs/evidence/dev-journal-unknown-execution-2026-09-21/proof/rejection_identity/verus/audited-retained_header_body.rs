        retained_indexed_access_v1($journal);
        if $writer.slot >= $journal.writers.len() {
            return Err(ReadErrorV1::InvalidReference);
        }
        let (key, head, count, unknown) = match $journal.writers[$writer.slot] {
            Some(WriterEntryV1::Pending { key, head, count }) => (key, head, count, false),
            Some(WriterEntryV1::Unknown { key, head, count }) if $allow_unknown => {
                (key, head, count, true)
            }
            _ => return Err(ReadErrorV1::InvalidReference),
        };
        if !shared_retained_writer_key_v1(key, $writer.key)
            || key.context_generation != $journal.context_generation
        {
            return Err(ReadErrorV1::InvalidReference);
        }
        Ok((head, count, unknown))

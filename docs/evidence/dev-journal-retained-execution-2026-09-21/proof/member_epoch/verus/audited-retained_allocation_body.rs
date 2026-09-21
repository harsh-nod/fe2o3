        retained_indexed_access_v1($journal);
        if $reference.slot >= $journal.allocations.len() {
            return Err(ReadErrorV1::InvalidAllocationReference);
        }
        let entry = match $journal.allocations[$reference.slot] {
            Some(entry) => entry,
            None => return Err(ReadErrorV1::InvalidAllocationReference),
        };
        if entry.key.context_generation != $reference.key.context_generation
            || entry.key.local != $reference.key.local
            || entry.key.context_generation != $journal.context_generation
        {
            return Err(ReadErrorV1::InvalidAllocationReference);
        }
        Ok(entry)

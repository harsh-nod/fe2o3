// Public allocation lookup validates only exact identity and the pending backlink.
 allocation_lookup_body {
    ($journal:ident, $reference:ident, $exact:path, $access:path) => {{
        let entry = match $exact($journal, $reference) {
            Ok(entry) => entry,
            Err(error) => return Err(error),
        };
        let pending_writer = match entry.pending_member {
            None => None,
            Some(slot) => {
                $access($journal);
                if slot >= $journal.members.len() {
                    return Err(ContextVersionJournalErrorV1::InvalidState);
                }
                let member = match $journal.members[slot] {
                    Some(member) => member,
                    None => return Err(ContextVersionJournalErrorV1::InvalidState),
                };
                if member.allocation.slot != $reference.slot
                    || member.allocation.key.context_generation != $reference.key.context_generation
                    || false
                {
                    return Err(ContextVersionJournalErrorV1::InvalidState);
                }
                Some(member.writer)
            }
        };
        Ok(ContextAllocationStateV1 {
            device: entry.device,
            byte_extent: entry.byte_extent,
            attempt_epoch: entry.attempt_epoch,
            content_lineage: entry.content_lineage,
            pending_writer,
        })
    }};
}

        let slot = match $head {
            Some(slot) => slot,
            None => return Err(ReadErrorV1::InvalidState),
        };
        retained_indexed_access_v1($journal);
        if slot >= $journal.members.len() {
            return Err(ReadErrorV1::InvalidState);
        }
        let member = match $journal.members[slot] {
            Some(member) => member,
            None => return Err(ReadErrorV1::InvalidState),
        };
        if member.writer.slot != $writer.slot
            || !shared_retained_writer_key_v1(member.writer.key, $writer.key)
        {
            return Err(ReadErrorV1::InvalidState);
        }
        if let Some(key) = $previous {
            if !shared_retained_allocation_less_v1(key, member.allocation.key) {
                return Err(ReadErrorV1::InvalidState);
            }
        }
        let allocation = match shared_retained_allocation_v1($journal, member.allocation) {
            Ok(allocation) => allocation,
            Err(_) => return Err(ReadErrorV1::InvalidState),
        };
        if false
            || allocation.attempt_epoch != member.attempt_epoch
            || allocation.content_lineage != member.prior_lineage
            || member.prior_lineage >= member.attempt_epoch
        {
            return Err(ReadErrorV1::InvalidState);
        }
        Ok(member)

// Shared executable admission/Unknown bodies. Loop annotations carry no runtime code.
macro_rules! retained_writer_key_body {
    ($left:ident, $right:ident) => {{
        $left.context_generation == $right.context_generation
            && $left.local == $right.local
            && match ($left.kind, $right.kind) {
                (WriterKindV1::Synchronous, WriterKindV1::Synchronous) => true,
                (WriterKindV1::Submission, WriterKindV1::Submission) => true,
                _ => false,
            }
    }};
}

macro_rules! retained_allocation_less_body {
    ($left:ident, $right:ident) => {{
        $left.context_generation > $right.context_generation
            || ($left.context_generation == $right.context_generation && $left.local < $right.local)
    }};
}

macro_rules! retained_allocation_body {
    ($journal:ident, $reference:ident) => {{
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
    }};
}

macro_rules! retained_header_body {
    ($journal:ident, $writer:ident, $allow_unknown:ident) => {{
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
    }};
}

macro_rules! retained_member_body {
    ($journal:ident, $writer:ident, $head:ident, $previous:ident) => {{
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
        if allocation.pending_member != Some(slot)
            || allocation.attempt_epoch != member.attempt_epoch
            || allocation.content_lineage != member.prior_lineage
            || member.prior_lineage >= member.attempt_epoch
        {
            return Err(ReadErrorV1::InvalidState);
        }
        Ok(member)
    }};
}

macro_rules! retained_chain_body {
    ($syntax:ident, $journal:ident, $writer:ident, $initial:ident, $count:ident,
     $head:ident, $previous:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $head = $initial;
            if $count > $journal.allocation_capacity || ($count == 0) != $head.is_none() {
                return Err(ReadErrorV1::InvalidState);
            }
            let mut $previous = None;
            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                let member = match shared_retained_member_v1($journal, $writer, $head, $previous) {
                    Ok(member) => member,
                    Err(error) => return Err(error),
                };
                $previous = Some(member.allocation.key);
                $head = member.next;
                $index += 1;
            }
            if $head.is_some() {
                return Err(ReadErrorV1::InvalidState);
            }
            Ok(())
        })
    };
}

macro_rules! retained_unknown_body {
    ($journal:ident, $writer:ident) => {{
        let (head, count, unknown) = match shared_retained_header_v1($journal, $writer, true) {
            Ok(header) => header,
            Err(error) => return Err(error),
        };
        let result = shared_retained_chain_v1($journal, $writer, head, count);
        if result.is_err() {
            return result;
        }
        if !unknown {
            retained_indexed_access_v1($journal);
            $journal.writers[$writer.slot] = Some(WriterEntryV1::Unknown {
                key: $writer.key,
                head,
                count,
            });
        }
        result
    }};
}

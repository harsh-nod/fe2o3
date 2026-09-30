// Complete local queued-read inspection, not a whole-list invariant check.
macro_rules! queued_read_reference_same_body_v1 {
    ($left:ident, $right:ident) => {
        $left.slot == $right.slot
            && $left.incarnation == $right.incarnation
            && queued_writer_key_same_v1($left.consumer, $right.consumer)
    };
}

macro_rules! queued_read_entry_body_v1 {
    ($owner:ident, $slot:ident) => {{
        if $slot >= $owner.queued_reads.len() {
            return Err(Error::InvalidReference);
        }
        match $owner.queued_reads[$slot] {
            Some(entry) => Ok(entry),
            None => Err(Error::InvalidReference),
        }
    }};
}

macro_rules! queued_read_links_body_v1 {
    ($owner:ident, $entry:ident) => {{
        if !matches!($entry.status, ContextProducerReadStatusV1::Pending) {
            return if $entry.previous.is_none() && $entry.next.is_none() {
                Ok(())
            } else {
                Err(Error::InvalidState)
            };
        }
        let root = $owner.root($entry.request.producer)?;
        if root.read_count == 0 || root.read_count > $owner.queued_reads.len() {
            return Err(Error::InvalidState);
        }
        if let Some(previous) = $entry.previous {
            let previous = $owner.read_entry(previous)?;
            if !queued_writer_same_v1(previous.request.producer, $entry.request.producer)
                || !matches!(previous.status, ContextProducerReadStatusV1::Pending)
                || match previous.next {
                    Some(slot) => slot != $entry.reference.slot,
                    None => true,
                }
            {
                return Err(Error::InvalidState);
            }
        } else if match root.read_head {
            Some(slot) => slot != $entry.reference.slot,
            None => true,
        } {
            return Err(Error::InvalidState);
        }
        if let Some(next) = $entry.next {
            let next = $owner.read_entry(next)?;
            if !queued_writer_same_v1(next.request.producer, $entry.request.producer)
                || !matches!(next.status, ContextProducerReadStatusV1::Pending)
                || match next.previous {
                    Some(slot) => slot != $entry.reference.slot,
                    None => true,
                }
            {
                return Err(Error::InvalidState);
            }
        }
        Ok(())
    }};
}

macro_rules! queued_read_inspect_body_v1 {
    ($owner:ident, $reference:ident) => {{
        $owner.ensure_usable()?;
        let entry = $owner.read_entry($reference.slot)?;
        if !queued_read_reference_same_v1(entry.reference, $reference) {
            return Err(Error::InvalidReference);
        }
        let state = $owner.destination(entry.request.allocation)?;
        let count = match $owner
            .read_counts
            .get(entry.request.allocation.allocation.slot)
        {
            Some(count) => *count,
            None => return Err(Error::InvalidReference),
        };
        if count == 0 {
            return Err(Error::InvalidState);
        }
        $owner.validate_read_links(entry)?;
        if matches!(entry.status, ContextProducerReadStatusV1::Success) {
            if match entry.version {
                Some((epoch, lineage)) => {
                    epoch != state.attempt_epoch || lineage != state.content_lineage
                }
                None => true,
            } || state.pending_writer.is_some()
                || state.attempt_epoch != state.content_lineage
            {
                return Err(Error::InvalidState);
            }
        } else if entry.version.is_some() {
            return Err(Error::InvalidState);
        }
        Ok(entry)
    }};
}

macro_rules! queued_read_lookup_body_v1 {
    ($owner:ident, $reference:ident) => {
        Ok($owner.inspect_queued_read($reference)?.request)
    };
}

macro_rules! queued_read_status_body_v1 {
    ($owner:ident, $reference:ident) => {
        Ok($owner.inspect_queued_read($reference)?.status)
    };
}

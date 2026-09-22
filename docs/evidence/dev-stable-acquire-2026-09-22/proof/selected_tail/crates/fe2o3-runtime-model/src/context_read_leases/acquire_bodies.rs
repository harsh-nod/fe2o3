// Normal-content acquisition; proof annotations erase without changing execution.
macro_rules! stable_read_capacity_body {
    ($contents:ident, $count:ident) => {{
        if $count > $contents.free_reads.len() {
            return Err(ContextVersionJournalErrorV1::MemberCapacity);
        }
        if $contents.next_incarnation == 0
            || $contents.next_incarnation.checked_add($count as u64).is_none()
        {
            return Err(ContextVersionJournalErrorV1::EpochExhausted);
        }
        Ok(())
    }};
}

macro_rules! stable_read_validate_body {
    ($contents:ident, $request:ident) => {{
        let state = match $contents.journal.lookup_allocation($request.allocation) {
            Ok(state) => state,
            Err(error) => return Err(error),
        };
        if state.device.context_generation != $request.device.context_generation
            || state.device.local != $request.device.local
        {
            return Err(ContextVersionJournalErrorV1::AllocationDeviceMismatch);
        }
        if state.byte_extent != $request.byte_extent {
            return Err(ContextVersionJournalErrorV1::AllocationExtentMismatch);
        }
        if $request.byte_len == 0 {
            return Err(ContextVersionJournalErrorV1::InvalidExtent);
        }
        let end = match $request.byte_offset.checked_add($request.byte_len) {
            Some(end) => end,
            None => return Err(ContextVersionJournalErrorV1::InvalidExtent),
        };
        if end > $request.byte_extent {
            return Err(ContextVersionJournalErrorV1::InvalidExtent);
        }
        if state.pending_writer.is_some() {
            return Err(ContextVersionJournalErrorV1::AllocationBusy);
        }
        if state.attempt_epoch != $request.attempt_epoch
            || state.content_lineage != $request.content_lineage
        {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        Ok(())
    }};
}

macro_rules! stable_read_output_vacant_body {
    ($syntax:ident, $output:ident, $index:ident, [$($invariant:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $output.len()
                $($invariant)*
            {
                if $output[$index].is_some() {
                    return false;
                }
                $index += 1;
            }
            true
        })
    };
}

macro_rules! stable_read_order_less_body {
    ($left:ident, $right:ident) => {
        $left.0 < $right.0 || ($left.0 == $right.0 &&
            ($left.1 < $right.1 || ($left.1 == $right.1 && $left.2 < $right.2)))
    };
}

macro_rules! stable_acquire_header_body {
    ($contents:ident, $consumer:ident, $count:ident, $output:ident) => {{
        if $consumer.context_generation != $contents.journal.context_generation() {
            return Err(ContextVersionJournalErrorV1::ForeignContext);
        }
        if $consumer.local == 0 || $consumer.local == u64::MAX {
            return Err(ContextVersionJournalErrorV1::InvalidWriterId);
        }
        if $count == 0 || $count != $output.len() {
            return Err(ContextVersionJournalErrorV1::RosterCapacity);
        }
        if !stable_read_output_vacant_exec_v1($output) {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        $contents.validate_read_capacity($count)
    }};
}

macro_rules! stable_acquire_item_body {
    ($contents:ident, $request:ident, $index:ident, $state:ident) => {{
        match $contents.validate_read(&$request) {
            Ok(_) => {},
            Err(error) => return Err(error),
        }
        let key = ($request.allocation.key.local, $request.byte_offset, $request.byte_len);
        if let Some(prior) = $state.previous {
            if !stable_read_order_less_exec_v1(prior, key) {
                return Err(ContextVersionJournalErrorV1::NonCanonicalRoster);
            }
        }
        let mut group = 1usize;
        if let Some(prior) = $state.previous {
            if prior.0 == key.0 {
                group = $state.group + 1;
            }
        }
        let count = match $contents.readers[$request.allocation.slot].checked_add(group) {
            Some(count) => count,
            None => return Err(ContextVersionJournalErrorV1::InvalidState),
        };
        if count > $contents.leases.len() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        let slot = $contents.free_reads[$index];
        if slot >= $contents.leases.len() || $contents.leases[slot].is_some() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        Ok(StableReadAcquireScanV1 { previous: Some(key), group })
    }};
}

macro_rules! stable_acquire_preflight_body {
    ($syntax:ident, $contents:ident, $consumer:ident, $requests:ident, $output:ident,
     $index:ident, $state:ident, [$($invariant:tt)*]) => {
        $syntax!({
            match stable_acquire_header_exec_v1($contents, $consumer, $requests.len(), $output) {
                Ok(_) => {},
                Err(error) => return Err(error),
            }
            let mut $index = 0usize;
            let mut $state = StableReadAcquireScanV1 { previous: None, group: 0 };
            while $index < $requests.len()
                $($invariant)*
            {
                match stable_acquire_item_exec_v1($contents, $requests[$index], $index, $state) {
                    Ok(next) => $state = next,
                    Err(error) => return Err(error),
                }
                $index += 1;
            }
            Ok(())
        })
    };
}

macro_rules! stable_acquire_commit_body {
    ($syntax:ident, $contents:ident, $consumer:ident, $requests:ident, $output:ident,
     $index:ident, [$($setup:tt)*], [$($invariant:tt)*],
     [$($count_proof:tt)*], [$($step_proof:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($setup)*
            let mut $index = 0usize;
            while $index < $requests.len()
                $($invariant)*
            {
                let slot = $contents.free_reads.pop().expect("preflighted free slot");
                let reference = ContextReadLeaseReferenceV1 {
                    slot,
                    incarnation: $contents.next_incarnation + $index as u64,
                    consumer: $consumer,
                };
                $contents.leases[slot] = Some(ReadLeaseV1 { reference, request: $requests[$index] });
                let allocation = $requests[$index].allocation.slot;
                $($count_proof)*
                $contents.readers[allocation] += 1;
                $output[$index] = Some(reference);
                $($step_proof)*
                $index += 1;
            }
            $($finish)*
            $contents.next_incarnation += $requests.len() as u64;
        })
    };
}

macro_rules! stable_acquire_execution_body {
    ($syntax:ident, $contents:ident, $consumer:ident, $requests:ident, $output:ident,
     $preflight:path, $commit:path, $value:ident,
     [$($setup:tt)*], [$($passed:tt)*], [$($ready:tt)*]) => {
        $syntax!({
            $($setup)*
            let _count = $requests.len();
            match $preflight($contents, $consumer, $requests, $output) {
                Ok($value) => { $($passed)* },
                Err(error) => return Err(error),
            }
            $($ready)*
            $commit($contents, $consumer, $requests, $output);
            Ok(())
        })
    };
}

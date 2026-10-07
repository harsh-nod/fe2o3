// Scalar admission deliberately differs from the batch enrollment contract.
macro_rules! scalar_enrollment_body {
    ($syntax:ident, $journal:ident, $key:ident, $device:ident, $extent:ident,
     $issuable:path, $count:path, $index:ident, [$($invariants:tt)*]) => {
        $syntax!({
            if $key.context_generation != $journal.context_generation {
                return Err(ContextVersionJournalErrorV1::ForeignContext);
            }
            if !$issuable($key.local) { return Err(ContextVersionJournalErrorV1::InvalidAllocationId); }
            if $device.context_generation != $journal.context_generation {
                return Err(ContextVersionJournalErrorV1::ForeignContext);
            }
            if !$issuable($device.local) { return Err(ContextVersionJournalErrorV1::InvalidDeviceId); }
            if $extent == 0 { return Err(ContextVersionJournalErrorV1::InvalidExtent); }
            let mut $index = 0usize;
            while $index < $journal.allocations.len()
                $($invariants)*
            {
                $count($journal);
                if let Some(entry) = $journal.allocations[$index] {
                    if entry.key.context_generation == $key.context_generation && entry.key.local == $key.local {
                        return Err(ContextVersionJournalErrorV1::AllocationReplay);
                    }
                }
                $index += 1;
            }
            $count($journal);
            if $journal.allocation_free.len() == 0 {
                return Err(ContextVersionJournalErrorV1::AllocationCapacity);
            }
            let slot = $journal.allocation_free[$journal.allocation_free.len() - 1];
            $count($journal);
            if slot >= $journal.allocations.len() || $journal.allocations[slot].is_some() {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            $count($journal);
            let _ = $journal.allocation_free.pop();
            $count($journal);
            $journal.allocations[slot] = Some(AllocationEntryV1 {
                key: $key, device: $device, byte_extent: $extent,
                attempt_epoch: 0, content_lineage: 0, pending_member: None,
            });
            Ok(ContextAllocationReferenceV1 { slot, key: $key })
        })
    };
}

macro_rules! scalar_enrollment_forward_body {
    ($owner:ident, $field:ident, $key:ident, $device:ident, $extent:ident) => {
        $owner.$field.enroll_allocation($key, $device, $extent)
    };
}

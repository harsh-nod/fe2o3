// Shared enrollment execution. Annotation hooks contain only proof/loop metadata.
macro_rules! enrollment_entry_error_body {
    ($context:ident, $entry:ident) => {{
        if $entry.key.context_generation != $context || $entry.device.context_generation != $context {
            Some(EnrollmentErrorV1::ForeignContext)
        } else if $entry.key.local == 0 || $entry.key.local == u64::MAX {
            Some(EnrollmentErrorV1::InvalidAllocationId)
        } else if $entry.device.local == 0 || $entry.device.local == u64::MAX {
            Some(EnrollmentErrorV1::InvalidDeviceId)
        } else if $entry.byte_extent == 0 {
            Some(EnrollmentErrorV1::InvalidExtent)
        } else { None }
    }};
}

macro_rules! enrollment_header_body {
    ($syntax:ident, $context:ident, $capacity:ident, $free_len:ident, $entries:ident, $output:ident,
     $index:ident, $previous:ident, [$($empty:tt)*], [$($roster:tt)*]) => {
        $syntax!({
            if $entries.len() > $capacity || $output.len() != $entries.len() {
                return Err(EnrollmentErrorV1::RosterCapacity);
            }
            let mut $index = 0usize;
            while $index < $output.len()
                $($empty)*
            {
                if $output[$index].is_some() { return Err(EnrollmentErrorV1::InvalidState); }
                $index += 1;
            }
            let mut $index = 0usize;
            let mut $previous = None;
            while $index < $entries.len()
                $($roster)*
            {
                let entry = $entries[$index];
                if let Some(error) = enrollment_entry_error_exec_v1($context, entry) {
                    return Err(error);
                }
                if let Some(key) = $previous {
                    if !enrollment_less(key, entry.key) {
                        return Err(EnrollmentErrorV1::NonCanonicalRoster);
                    }
                }
                $previous = Some(entry.key);
                $index += 1;
            }
            if $entries.len() == 0 { return Ok(()); }
            if $free_len > $capacity { return Err(EnrollmentErrorV1::InvalidState); }
            Ok(())
        })
    };
}

macro_rules! enrollment_fill_plan_body {
    ($syntax:ident, $journal:ident, $entries:ident, $output:ident, $index:ident,
     [$($invariants:tt)*], [$($filled:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $entries.len()
                $($invariants)*
            {
                let slot = $journal.allocation_free[$journal.allocation_free.len() - 1 - $index];
                $output[$index] = Some(AllocationReferenceV1 { slot, key: $entries[$index].key });
                $index += 1;
            }
            $($filled)*
        })
    };
}

macro_rules! enrollment_clear_output_body {
    ($syntax:ident, $output:ident, $index:ident, [$($invariants:tt)*], [$($cleared:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $output.len()
                $($invariants)*
            {
                $output[$index] = None;
                $index += 1;
            }
            $($cleared)*
        })
    };
}

macro_rules! enrollment_duplicate_slots_body {
    ($syntax:ident, $values:ident, $index:ident, [$($invariants:tt)*], [$($distinct:tt)*]) => {
        $syntax!({
            if $values.len() == 0 { return false; }
            let mut $index = 1usize;
            while $index < $values.len()
                $($invariants)*
            {
                if $values[$index - 1].unwrap().slot == $values[$index].unwrap().slot { return true; }
                $index += 1;
            }
            $($distinct)*
            false
        })
    };
}

macro_rules! enrollment_replay_body {
    ($syntax:ident, $journal:ident, $entries:ident, $index:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $journal.allocations.len()
                $($invariants)*
            {
                enrollment_indexed_access_v1($journal);
                if let Some(entry) = $journal.allocations[$index] {
                    if contains_key($entries, entry.key) { return true; }
                }
                $index += 1;
            }
            false
        })
    };
}

macro_rules! enrollment_selected_vacant_body {
    ($syntax:ident, $journal:ident, $remaining:ident, $index:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let mut $index = $remaining;
            while $index < $journal.allocation_free.len()
                $($invariants)*
            {
                let slot = $journal.allocation_free[$index];
                if slot >= $journal.allocations.len() || $journal.allocations[slot].is_some() { return false; }
                $index += 1;
            }
            true
        })
    };
}

macro_rules! enrollment_retained_clear_body {
    ($syntax:ident, $journal:ident, $values:ident, $remaining:ident, $index:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $remaining
                $($invariants)*
            {
                if contains_slot($values, $journal.allocation_free[$index]) { return false; }
                $index += 1;
            }
            true
        })
    };
}

macro_rules! enrollment_commit_journal_body {
    ($syntax:ident, $contents:ident, $entries:ident, $output:ident, $remaining:ident,
     $index:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $entries.len()
                $($invariants)*
            {
                let entry = $entries[$index];
                let reference = $output[$index].unwrap();
                $contents.allocations[reference.slot] = Some(AllocationEntryV1 {
                    key: entry.key, device: entry.device, byte_extent: entry.byte_extent,
                    attempt_epoch: 0, content_lineage: 0, pending_member: None,
                });
                $index += 1;
            }
            $contents.allocation_free.truncate($remaining);
        })
    };
}

macro_rules! enrollment_journal_body {
    ($syntax:ident, $contents:ident, $entries:ident, $output:ident, $remaining:ident,
     [$($header:tt)*], [$($empty:tt)*], [$($planned:tt)*], [$($sorted:tt)*],
     [$($cleared:tt)*], [$($refilled:tt)*]) => {
        $syntax!({
            if let Err(error) = enrollment_header_exec_v1($contents.context_generation,
                $contents.allocation_capacity, $contents.allocation_free.len(), $entries, $output) {
                return Err(error);
            }
            $($header)*
            if $entries.len() == 0 {
                $($empty)*
                return Ok(());
            }
            if enrollment_replay_exec_v1($contents, $entries) { return Err(EnrollmentErrorV1::AllocationReplay); }
            if $entries.len() > $contents.allocation_free.len() { return Err(EnrollmentErrorV1::AllocationCapacity); }
            let $remaining = $contents.allocation_free.len() - $entries.len();
            if !enrollment_selected_vacant_exec_v1($contents, $remaining) { return Err(EnrollmentErrorV1::InvalidState); }
            // Only inert caller output changes before all fallible checks finish.
            enrollment_fill_plan_exec_v1($contents, $entries, $output);
            $($planned)*
            adaptive_sort_slots($output);
            $($sorted)*
            if enrollment_duplicate_slots_exec_v1($output) || !enrollment_retained_clear_exec_v1($contents, $output, $remaining) {
                enrollment_clear_output_exec_v1($output);
                $($cleared)*
                return Err(EnrollmentErrorV1::InvalidState);
            }
            // Restore reverse-stack/key associations before committing selected slots.
            let _ = $output.len();
            $($refilled)*
            enrollment_commit_journal_exec_v1($contents, $entries, $output, $remaining);
            Ok(())
        })
    };
}

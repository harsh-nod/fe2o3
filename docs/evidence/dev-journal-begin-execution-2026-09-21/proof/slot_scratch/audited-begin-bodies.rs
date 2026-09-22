// Shared Begin execution. Annotation hooks contain only proof/loop metadata.
 begin_reserved_body {
    ($journal:ident, $writer:ident) => {{
        begin_indexed_access_v1($journal);
        if $writer.slot >= $journal.writers.len() { return Err(ReadErrorV1::InvalidReference); }
        match $journal.writers[$writer.slot] {
            Some(WriterEntryV1::Reserved(key))
                if shared_retained_writer_key_v1(key, $writer.key)
                    && key.context_generation == $journal.context_generation => Ok(key),
            _ => Err(ReadErrorV1::InvalidReference),
        }
    }};
}

 begin_canonical_body {
    ($syntax:ident, $roster:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            if $roster.len() == 0 { return Ok(()); }
            let mut $index = 1usize;
            while $index < $roster.len()
                $($annotations)*
            {
                if !shared_retained_allocation_less_v1(
                    $roster[$index - 1].allocation.key, $roster[$index].allocation.key) {
                    return Err(ReadErrorV1::NonCanonicalRoster);
                }
                $index += 1;
            }
            Ok(())
        })
    };
}

 begin_destinations_body {
    ($syntax:ident, $journal:ident, $roster:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $roster.len()
                $($annotations)*
            {
                let destination = $roster[$index];
                let entry = match shared_retained_allocation_v1($journal, destination.allocation) {
                    Ok(entry) => entry, Err(error) => return Err(error),
                };
                if entry.device.context_generation != destination.device.context_generation
                    || entry.device.local != destination.device.local {
                    return Err(ReadErrorV1::AllocationDeviceMismatch);
                }
                if entry.byte_extent != destination.byte_extent { return Err(ReadErrorV1::AllocationExtentMismatch); }
                if entry.pending_member.is_some() { return Err(ReadErrorV1::AllocationBusy); }
                if entry.attempt_epoch.checked_add(1).is_none() { return Err(ReadErrorV1::EpochExhausted); }
                $index += 1;
            }
            Ok(())
        })
    };
}

 begin_slots_body {
    ($syntax:ident, $journal:ident, $count:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                begin_indexed_access_v1($journal);
                let member = $journal.member_free[$journal.member_free.len() - 1 - $index];
                begin_indexed_access_v1($journal);
                if member >= $journal.members.len() || $journal.members[member].is_some() {
                    return Err(ReadErrorV1::InvalidState);
                }
                begin_indexed_access_v1($journal);
                if false { return Err(ReadErrorV1::InvalidState); }
                $index += 1;
            }
            Ok(())
        })
    };
}

 begin_preflight_body {
    ($journal:ident, $writer:ident, $roster:ident) => {{
        if let Err(error) = begin_reserved_exec_v1($journal, $writer) { return Err(error); }
        let reserved_count = match $journal.reserved_count.checked_sub(1) {
            Some(count) => count, None => return Err(ReadErrorV1::InvalidState),
        };
        let count = $roster.len();
        if count > $journal.allocation_capacity { return Err(ReadErrorV1::RosterCapacity); }
        if let Err(error) = begin_canonical_exec_v1($roster) { return Err(error); }
        if let Err(error) = begin_destinations_exec_v1($journal, $roster) { return Err(error); }
        if count > $journal.member_free.len() { return Err(ReadErrorV1::MemberCapacity); }
        if count > $journal.scratch.len() { return Err(ReadErrorV1::InvalidState); }
        if let Err(error) = begin_slots_exec_v1($journal, count) { return Err(error); }
        Ok(reserved_count)
    }};
}

 begin_stage_body {
    ($syntax:ident, $journal:ident, $roster:ident, $index:ident,
     [$($annotations:tt)*], [$($advanced:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $roster.len()
                $($annotations)*
            {
                let destination = $roster[$index];
                begin_indexed_access_v1($journal);
                let entry = $journal.allocations[destination.allocation.slot]
                    .as_ref().expect("whole-roster preflight retains exact allocations");
                begin_indexed_access_v1($journal);
                let plan = BeginMemberPlanV1 {
                    member_slot: $journal.member_free[$journal.member_free.len() - 1 - $index],
                    allocation: destination.allocation, prior_lineage: entry.content_lineage,
                    attempt_epoch: entry.attempt_epoch + 1,
                };
                begin_indexed_access_v1($journal);
                $journal.scratch[$index] = Some(plan);
                $index += 1;
                $($advanced)*
            }
        })
    };
}

 begin_commit_body {
    ($syntax:ident, $journal:ident, $writer:ident, $roster:ident, $reserved_count:ident,
     $index:ident, $count:ident, $head:ident, [$($annotations:tt)*], [$($advanced:tt)*]) => {
        $syntax!({
            let $count = $roster.len();
            let $head = if $count == 0 { None } else {
                begin_indexed_access_v1($journal);
                Some($journal.scratch[0].expect("complete member plan").member_slot)
            };
            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                begin_indexed_access_v1($journal);
                let plan = $journal.scratch[$index].take().expect("complete member plan");
                let next = if $index + 1 == $count { None } else {
                    begin_indexed_access_v1($journal);
                    Some($journal.scratch[$index + 1].expect("complete next member plan").member_slot)
                };
                begin_indexed_access_v1($journal);
                let _ = $journal.member_free.pop();
                begin_indexed_access_v1($journal);
                $journal.members[plan.member_slot] = Some(MemberEntryV1 {
                    writer: $writer, allocation: plan.allocation,
                    prior_lineage: plan.prior_lineage, attempt_epoch: plan.attempt_epoch, next,
                });
                begin_indexed_access_v1($journal);
                {
                    let entry = $journal.allocations[plan.allocation.slot]
                        .as_mut().expect("retained exact allocation");
                    entry.attempt_epoch = plan.attempt_epoch;
                    entry.pending_member = Some(plan.member_slot);
                }
                $index += 1;
                $($advanced)*
            }
            begin_indexed_access_v1($journal);
            $journal.writers[$writer.slot] = Some(WriterEntryV1::Pending { key: $writer.key, head: $head, count: $count });
            $journal.reserved_count = $reserved_count;
        })
    };
}

 begin_execution_body {
    ($syntax:ident, $journal:ident, $writer:ident, $roster:ident, $reserved_count:ident,
     [$($ready:tt)*], [$($ghost_before:tt)*]) => {
        $syntax!({
            let $reserved_count = match begin_preflight_exec_v1($journal, $writer, $roster) {
                Ok(count) => count, Err(error) => return Err(error),
            };
            $($ready)*
            begin_stage_exec_v1($journal, $roster);
            begin_commit_exec_v1($journal, $writer, $roster, $reserved_count $($ghost_before)*);
            Ok(())
        })
    };
}

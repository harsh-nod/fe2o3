// Capacity expressions remain lazy; annotations supply only proof metadata.
macro_rules! disposal_plan_body {
    ($syntax:ident, $journal:ident, $writer:ident, $roster:ident,
     $writer_capacity:expr, $member_capacity:expr, $allocation_capacity:expr,
     $header:path, $chain:path, $exact:path, $scratch:path,
     $head:ident, $count:ident, $cursor:ident, $index:ident,
     [$($ready:tt)*], [$($annotations:tt)*], [$($step:tt)*]) => {
        $syntax!({
            let ($head, $count, unknown) = match $header($journal, $writer, true) {
                Ok(value) => value, Err(error) => return Err(error),
            };
            if !unknown { return Err(ContextVersionJournalErrorV1::InvalidState); }
            if $roster.len() != $count { return Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch); }
            if let Err(error) = $chain($journal, $writer, $head, $count) { return Err(error); }
            $($ready)*
            let mut $cursor = $head;
            let mut $index = 0usize;
            while $index < $roster.len()
                $($annotations)*
            {
                $($step)*
                let destination = $roster[$index];
                let slot = match $cursor {
                    Some(value) => value, None => return Err(ContextVersionJournalErrorV1::InvalidState),
                };
                let member = match $journal.members[slot] {
                    Some(value) => value, None => return Err(ContextVersionJournalErrorV1::InvalidState),
                };
                let allocation = match $exact($journal, member.allocation) {
                    Ok(value) => value, Err(error) => return Err(error),
                };
                if destination.allocation.slot != member.allocation.slot
                    || destination.allocation.key.context_generation != member.allocation.key.context_generation
                    || destination.allocation.key.local != member.allocation.key.local
                    || destination.device.context_generation != allocation.device.context_generation
                    || destination.device.local != allocation.device.local
                    || destination.byte_extent != allocation.byte_extent
                { return Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch); }
                $cursor = member.next;
                $index += 1;
            }
            let writer_returns = match $journal.free.len().checked_add(1) {
                Some(value) => value, None => return Err(ContextVersionJournalErrorV1::InvalidState),
            };
            let member_returns = match $journal.member_free.len().checked_add($count) {
                Some(value) => value, None => return Err(ContextVersionJournalErrorV1::InvalidState),
            };
            let allocation_returns = match $journal.allocation_free.len().checked_add($count) {
                Some(value) => value, None => return Err(ContextVersionJournalErrorV1::InvalidState),
            };
            if writer_returns > $journal.writer_capacity || writer_returns > $writer_capacity
                || member_returns > $journal.allocation_capacity || member_returns > $member_capacity
                || allocation_returns > $journal.allocation_capacity || allocation_returns > $allocation_capacity
                || $count > $journal.scratch.len()
            { return Err(ContextVersionJournalErrorV1::InvalidState); }
            if let Err(error) = $scratch($journal, $count) { return Err(error); }
            Ok(($head, $count))
        })
    };
}

macro_rules! disposal_scratch_scan_body {
    ($syntax:ident, $journal:ident, $count:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                if $journal.scratch[$index].is_some() { return Err(ContextVersionJournalErrorV1::InvalidState); }
                $index += 1;
            }
            Ok(())
        })
    };
}

macro_rules! disposal_stage_body {
    ($syntax:ident, $journal:ident, $initial:ident, $count:ident, $access:path,
     $head:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $head = $initial;
            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                let slot = $head.expect("validated complete Unknown chain");
                let member = $journal.members[slot].expect("validated Unknown member");
                $access($journal);
                $journal.scratch[$index] = Some(BeginMemberPlanV1 {
                    member_slot: slot, allocation: member.allocation,
                    prior_lineage: member.prior_lineage, attempt_epoch: member.attempt_epoch,
                });
                $head = member.next;
                $index += 1;
            }
        })
    };
}

macro_rules! disposal_commit_body {
    ($syntax:ident, $journal:ident, $writer:ident, $count:ident, $access:path,
     $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                let plan = $journal.scratch[$index].take().expect("complete disposal plan");
                $journal.allocations[plan.allocation.slot] = None;
                $journal.allocation_free.push(plan.allocation.slot);
                $journal.members[plan.member_slot] = None;
                $journal.member_free.push(plan.member_slot);
                $index += 1;
            }
            $access($journal);
            $journal.writers[$writer.slot] = None;
            $access($journal);
            $journal.free.push($writer.slot);
        })
    };
}

macro_rules! disposal_validate_body {
    ($journal:ident, $writer:ident, $roster:ident, $plan:ident, [$($extra:tt)*]) => {
        match $journal.$plan($writer, $roster $($extra)*) {
            Ok(_) => Ok(()), Err(error) => Err(error),
        }
    };
}

macro_rules! disposal_execute_body {
    ($syntax:ident, $journal:ident, $writer:ident, $evidence:ident,
     $header:path, $same_key:path, $plan:ident, [$($observations:tt)*], $stage:path, $commit:path,
     $head:ident, $count:ident, [$($commit_extra:tt)*], [$($before:tt)*], [$($ready:tt)*]) => {
        $syntax!({
            $($before)*
            if let Err(error) = $header($journal, $writer, true) { return Err(error); }
            if $evidence.writer.slot != $writer.slot || !$same_key($evidence.writer.key, $writer.key)
            { return Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch); }
            let ($head, $count) = match $journal.$plan($writer, $evidence.allocations $($observations)*) {
                Ok(value) => value, Err(error) => return Err(error),
            };
            $($ready)*
            $stage($journal, $head, $count);
            $commit($journal, $writer, $count $($commit_extra)*);
            Ok(())
        })
    };
}

macro_rules! disposal_owner_validate_body {
    ($syntax:ident, $owner:ident, $field:ident, $writer:ident, $roster:ident,
     $unread:ident, $validate:ident, [$($observations:tt)*], [$($ready:tt)*]) => {
        $syntax!({
            if let Err(error) = $owner.$unread($roster) { return Err(error); }
            $($ready)*
            $owner.$field.$validate($writer, $roster $($observations)*)
        })
    };
}

macro_rules! disposal_owner_execute_body {
    ($syntax:ident, $owner:ident, $field:ident, $writer:ident, $evidence:ident,
     $validate:ident, $execute:ident, [$($observations:tt)*], [$($ready:tt)*]) => {
        $syntax!({
            if let Err(error) = $owner.$validate($writer, $evidence.allocations $($observations)*) {
                return Err(error);
            }
            $($ready)*
            $owner.$field.$execute($writer, $evidence $($observations)*)
        })
    };
}

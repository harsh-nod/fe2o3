// Normal-content retirement; capacity remains a lazy observation after admission.
macro_rules! retirement_preflight_body {
    ($syntax:ident, $journal:ident, $roster:ident, $exact:path, $less:path, $capacity:expr,
     $previous:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            if $roster.len() > $journal.allocation_capacity {
                return Err(ContextVersionJournalErrorV1::RosterCapacity);
            }
            let mut $previous = None;
            let mut $index = 0usize;
            while $index < $roster.len()
                $($annotations)*
            {
                let reference = $roster[$index];
                let entry = match $exact($journal, reference) {
                    Ok(entry) => entry,
                    Err(error) => return Err(error),
                };
                if let Some(key) = $previous {
                    if !$less(key, reference.key) {
                        return Err(ContextVersionJournalErrorV1::NonCanonicalRoster);
                    }
                }
                if entry.pending_member.is_some() {
                    return Err(ContextVersionJournalErrorV1::AllocationBusy);
                }
                $previous = Some(reference.key);
                $index += 1;
            }
            let returned = match $journal.allocation_free.len().checked_add($roster.len()) {
                Some(count) => count,
                None => return Err(ContextVersionJournalErrorV1::InvalidState),
            };
            if returned > $journal.allocation_capacity || returned > $capacity {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            Ok(())
        })
    };
}

macro_rules! retirement_execute_body {
    ($syntax:ident, $journal:ident, $roster:ident, $validate:ident, [$($observations:tt)*],
     $index:ident, [$($setup:tt)*], [$($annotations:tt)*], [$($step:tt)*]) => {
        $syntax!({
            match $journal.$validate($roster $($observations)*) {
                Ok(()) => {},
                Err(error) => return Err(error),
            }
            $($setup)*
            let mut $index = 0usize;
            while $index < $roster.len()
                $($annotations)*
            {
                $($step)*
                let reference = $roster[$index];
                $journal.allocations[reference.slot] = None;
                $journal.allocation_free.push(reference.slot);
                $index += 1;
            }
            Ok(())
        })
    };
}

macro_rules! retirement_unread_body {
    ($syntax:ident, $owner:ident, $roster:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $roster.len()
                $($annotations)*
            {
                let count = match $owner.reader_count($roster[$index]) {
                    Ok(count) => count,
                    Err(error) => return Err(error),
                };
                if count != 0 {
                    return Err(ContextVersionJournalErrorV1::AllocationBusy);
                }
                $index += 1;
            }
            Ok(())
        })
    };
}

macro_rules! retirement_owner_validate_body {
    ($syntax:ident, $owner:ident, $field:ident, $roster:ident, $validate:ident, [$($observations:tt)*], [$($proof:tt)*]) => {
        $syntax!({
        match $owner.require_unread_allocations($roster) {
            Ok(()) => {},
            Err(error) => return Err(error),
        }
        $($proof)*
        $owner.$field.$validate($roster $($observations)*)
        })
    };
}

macro_rules! retirement_owner_execute_body {
    ($syntax:ident, $owner:ident, $field:ident, $roster:ident, $validate:ident, $retire:ident,
     [$($observations:tt)*], [$($proof:tt)*]) => {
        $syntax!({
        match $owner.$validate($roster $($observations)*) {
            Ok(()) => {},
            Err(error) => return Err(error),
        }
        $($proof)*
        $owner.$field.$retire($roster $($observations)*)
        })
    };
}

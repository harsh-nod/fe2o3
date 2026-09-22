// Capacity is observed only after the original early-return header prefix.
macro_rules! stable_read_reference_same_body {
    ($left:ident, $right:ident) => {
        $left.slot == $right.slot && $left.incarnation == $right.incarnation
            && stable_read_consumer_same_exec_v1($left.consumer, $right.consumer)
    };
}

macro_rules! stable_lease_lookup_body {
    ($contents:ident, $reference:ident, $same:path) => {{
        if $reference.slot >= $contents.leases.len() {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        let entry = match $contents.leases[$reference.slot] {
            Some(entry) => entry,
            None => return Err(ContextVersionJournalErrorV1::InvalidReference),
        };
        if !$same(entry.reference, $reference) {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        match $contents.validate_read(&entry.request) {
            Ok(_) => Ok(entry.request),
            Err(error) => Err(error),
        }
    }};
}

macro_rules! stable_release_order_less_body {
    ($left:ident, $right:ident) => {
        $left.0 < $right.0 || ($left.0 == $right.0 &&
            ($left.1 < $right.1 || ($left.1 == $right.1 &&
                ($left.2 < $right.2 || ($left.2 == $right.2 && $left.3 < $right.3)))))
    };
}

macro_rules! stable_release_header_body {
    ($contents:ident, $consumer:ident, $evidence:ident, $count:ident, $observe_capacity:expr) => {{
        if !stable_read_consumer_same_exec_v1($evidence, $consumer) {
            return Err(ContextVersionJournalErrorV1::SettlementEvidenceMismatch);
        }
        if $count == 0 {
            return Err(ContextVersionJournalErrorV1::RosterCapacity);
        }
        let count = match $contents.free_reads.len().checked_add($count) {
            Some(count) => count,
            None => return Err(ContextVersionJournalErrorV1::InvalidState),
        };
        if count > $contents.leases.len() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        if count > $observe_capacity {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        Ok(())
    }};
}

macro_rules! stable_release_item_body {
    ($contents:ident, $consumer:ident, $reference:ident, $state:ident) => {{
        if !stable_read_consumer_same_exec_v1($reference.consumer, $consumer) {
            return Err(ContextVersionJournalErrorV1::InvalidReference);
        }
        let request = match $contents.lookup_read($reference) {
            Ok(request) => request,
            Err(error) => return Err(error),
        };
        let key = (request.allocation.key.local, request.byte_offset, request.byte_len, $reference.incarnation);
        if let Some(prior) = $state.previous {
            if !stable_release_order_less_exec_v1(prior, key) {
                return Err(ContextVersionJournalErrorV1::NonCanonicalRoster);
            }
        }
        let mut group = 1usize;
        if let Some(prior) = $state.previous {
            if prior.0 == key.0 { group = $state.group + 1; }
        }
        if $contents.readers[request.allocation.slot] < group {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        Ok(StableReadReleaseScanV1 { previous: Some(key), group })
    }};
}

macro_rules! stable_release_preflight_body {
    ($syntax:ident, $contents:ident, $consumer:ident, $references:ident, $evidence:ident,
     [$($capacity_arg:tt)*], $index:ident, $state:ident, [$($invariant:tt)*]) => {
        $syntax!({
            match stable_release_header_exec_v1($contents, $consumer, $evidence, $references.len() $($capacity_arg)*) {
                Ok(_) => {},
                Err(error) => return Err(error),
            }
            let mut $index = 0usize;
            let mut $state = StableReadReleaseScanV1 { previous: None, group: 0 };
            while $index < $references.len()
                $($invariant)*
            {
                match stable_release_item_exec_v1($contents, $consumer, $references[$index], $state) {
                    Ok(next) => $state = next,
                    Err(error) => return Err(error),
                }
                $index += 1;
            }
            Ok(())
        })
    };
}

macro_rules! stable_release_commit_body {
    ($syntax:ident, $contents:ident, $references:ident, $index:ident, $reference:ident,
     $entry:ident, $allocation:ident, [$($setup:tt)*], [$($invariant:tt)*],
     [$($selected:tt)*], [$($count:tt)*], [$($step:tt)*]) => {
        $syntax!({
            $($setup)*
            let mut $index = 0usize;
            while $index < $references.len()
                $($invariant)*
            {
                let $reference = $references[$index];
                $($selected)*
                let $entry = $contents.leases[$reference.slot].take().expect("preflighted lease");
                let $allocation = $entry.request.allocation.slot;
                $($count)*
                $contents.readers[$allocation] -= 1;
                $contents.free_reads.push(0);
                $($step)*
                $index += 1;
            }
        })
    };
}

macro_rules! stable_release_execution_body {
    ($syntax:ident, $contents:ident, $consumer:ident, $references:ident, $evidence:ident,
     $preflight:path, $commit:path, [$($capacity_arg:tt)*], $value:ident,
     [$($setup:tt)*], [$($passed:tt)*], [$($ready:tt)*]) => {
        $syntax!({
            $($setup)*
            let _count = $references.len();
            match $preflight($contents, $consumer, $references, $evidence.consumer $($capacity_arg)*) {
                Ok($value) => { $($passed)* },
                Err(error) => return Err(error),
            }
            $($ready)*
            $commit($contents, $references);
            Ok(())
        })
    };
}

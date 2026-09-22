macro_rules! producer_retained_count_body {
    ($contents:ident) => { $contents.reservations.len() - $contents.free.len() };
}

macro_rules! producer_total_read_count_body {
    ($contents:ident) => { $contents.stable.retained_read_count() + $contents.retained_producer_read_count() };
}

macro_rules! producer_remaining_read_slots_body {
    ($contents:ident) => { $contents.reservations.len() - $contents.retained_read_count() };
}

macro_rules! producer_stable_capacity_body {
    ($contents:ident, $count:ident) => {{
        if $count > $contents.remaining_read_slots() {
            return Err(ContextVersionJournalErrorV1::MemberCapacity);
        }
        $contents.stable.validate_read_capacity($count)
    }};
}

macro_rules! producer_capacity_body {
    ($contents:ident, $count:ident) => {{
        if $count > $contents.remaining_read_slots() {
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

macro_rules! producer_acquire_header_body {
    ($contents:ident, $generation:expr, $consumer:ident, $count:ident, $output:ident) => {{
        if $consumer.context_generation != $generation {
            return Err(ContextVersionJournalErrorV1::ForeignContext);
        }
        if $consumer.local == 0 || $consumer.local == u64::MAX
            || !matches!($consumer.kind, ContextWriterKindV1::Submission)
        {
            return Err(ContextVersionJournalErrorV1::InvalidWriterId);
        }
        if $count == 0 || $count != $output.len() {
            return Err(ContextVersionJournalErrorV1::RosterCapacity);
        }
        if !producer_output_vacant_exec_v1($output) {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        $contents.validate_producer_read_capacity($count)
    }};
}

macro_rules! producer_acquire_item_body {
    ($contents:ident, $consumer:ident, $request:ident, $index:ident, $state:ident) => {{
        match $contents.validate_producer_read(&$request) {
            Ok(_) => {},
            Err(error) => return Err(error),
        }
        if $request.producer.key.local >= $consumer.local {
            return Err(ContextVersionJournalErrorV1::InvalidWriterId);
        }
        let key = ($request.read.allocation.key.local, $request.read.byte_offset, $request.read.byte_len);
        if let Some(prior) = $state.previous {
            if !producer_read_order_less_exec_v1(prior, key) {
                return Err(ContextVersionJournalErrorV1::NonCanonicalRoster);
            }
        }
        let mut group = 1usize;
        if let Some(prior) = $state.previous {
            if prior.0 == key.0 {
                group = $state.group + 1;
            }
        }
        let count = match $contents.counts[$request.read.allocation.slot].checked_add(group) {
            Some(count) => count,
            None => return Err(ContextVersionJournalErrorV1::InvalidState),
        };
        if count > $contents.reservations.len() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        let slot = $contents.free[$index];
        if slot >= $contents.reservations.len() || $contents.reservations[slot].is_some() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        Ok(ProducerReadAcquireScanV1 { previous: Some(key), group })
    }};
}

macro_rules! producer_acquire_preflight_body {
    ($syntax:ident, $contents:ident, $consumer:ident, $requests:ident, $output:ident,
     $index:ident, $state:ident, [$($invariant:tt)*]) => {
        $syntax!({
            match producer_acquire_header_exec_v1($contents, $consumer, $requests.len(), $output) {
                Ok(_) => {},
                Err(error) => return Err(error),
            }
            let mut $index = 0usize;
            let mut $state = ProducerReadAcquireScanV1 { previous: None, group: 0 };
            while $index < $requests.len()
                $($invariant)*
            {
                match producer_acquire_item_exec_v1($contents, $consumer, $requests[$index], $index, $state) {
                    Ok(next) => $state = next,
                    Err(error) => return Err(error),
                }
                $index += 1;
            }
            Ok(())
        })
    };
}

macro_rules! producer_acquire_commit_body {
    ($syntax:ident, $contents:ident, $consumer:ident, $requests:ident, $output:ident,
     $index:ident, [$($setup:tt)*], [$($invariant:tt)*],
     [$($count_proof:tt)*], [$($step_proof:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($setup)*
            let mut $index = 0usize;
            while $index < $requests.len()
                $($invariant)*
            {
                let slot = $contents.free.pop().expect("preflighted free reservation");
                let reference = ContextProducerReadReferenceV1 {
                    slot,
                    incarnation: $contents.next_incarnation + $index as u64,
                    consumer: $consumer,
                };
                $contents.reservations[slot] = Some(ReservationV1 { reference, request: $requests[$index] });
                let allocation = $requests[$index].read.allocation.slot;
                $($count_proof)*
                $contents.counts[allocation] += 1;
                $output[$index] = Some(reference);
                $($step_proof)*
                $index += 1;
            }
            $($finish)*
            $contents.next_incarnation += $requests.len() as u64;
        })
    };
}

macro_rules! producer_acquire_execution_body {
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

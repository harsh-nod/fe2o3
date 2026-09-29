macro_rules! completion_issue_result {
    ($result:expr) => {
        $result
    };
}

macro_rules! completion_dependency_ledger_new_body {
    ($syntax:ident) => {
        $syntax!(Self {
            next_event_id: 1,
            next_reader_lease_id: 1,
            events: HashMap::new(),
            readers: HashMap::new(),
        })
    };
}

macro_rules! completion_logical_identity_body {
    ($syntax:ident, $session:ident, $epoch:ident) => {
        $syntax!({
            if $session == 0 {
                return Err(Gfx942CompletionErrorV1::InvalidSessionOccurrence);
            }
            if $epoch == 0 {
                return Err(Gfx942CompletionErrorV1::InvalidAcceptanceEpoch);
            }
            Ok(())
        })
    };
}

macro_rules! completion_record_event_batch_body {
    ($syntax:ident, $owner:ident, $session:ident, $epoch:ident, $retention:ident, $n:ident) => {
        completion_record_event_batch_body!(@annotated $syntax, $owner, $session, $epoch, $retention, $n,
            completion_issue_result, i, events, reservation, next_event_id, occurrence,
            [], [], [], [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $session:ident, $epoch:ident, $retention:ident, $n:ident,
        $finish:ident, $i:ident, $events:ident, $reservation:ident, $next:ident, $occurrence:ident,
        [$($start:tt)*], [$($preflight:tt)*], [$($checked:tt)*], [$($ledger_reserved:tt)*],
        [$($output_reserved:tt)*], [$($precommit:tt)*], [$($commit:tt)*], [$($writing:tt)*], [$($written:tt)*]) => {
        $syntax!({
            $($start)*
            if let Err(error) = $owner.require_ready() {
                return $finish!(Err(error));
            }
            if let Err(error) = $owner.validate_bound($retention) {
                return $finish!(Err(error));
            }
            if let Err(error) = validate_logical_identity($session, $epoch) {
                return $finish!(Err(error));
            }
            let Some(next_len) = $owner.dependency_ledger.events.len().checked_add($n) else {
                return $finish!(Err(Gfx942CompletionErrorV1::EventCapacityExhausted));
            };
            if next_len > GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1 {
                return $finish!(Err(Gfx942CompletionErrorV1::EventCapacityExhausted));
            }
            let count = match u64::try_from($n) {
                Ok(count) => count,
                Err(_error) => return $finish!(Err(Gfx942CompletionErrorV1::EventIdentityExhausted)),
            };
            let Some($next) = $owner.dependency_ledger.next_event_id.checked_add(count) else {
                return $finish!(Err(Gfx942CompletionErrorV1::EventIdentityExhausted));
            };
            let mut $i = 0;
            while $i < $n $($preflight)* {
                let $occurrence = match exact_occurrence($session, $epoch, $retention, $i, None) {
                    Ok(occurrence) => occurrence,
                    Err(error) => return $finish!(Err(error)),
                };
                if $owner.slots[$occurrence.slot.index as usize].event_pins.checked_add(1).is_none() {
                    return $finish!(Err(Gfx942CompletionErrorV1::SignalPinCountExhausted));
                }
                $($checked)*
                $i += 1;
            }
            let $reservation = $owner.dependency_ledger.events.try_reserve($n);
            $($ledger_reserved)*
            if $reservation.is_err() {
                return $finish!(Err(Gfx942CompletionErrorV1::DependencyLedgerAllocation));
            }
            let mut $events = Vec::new();
            let $reservation = $events.try_reserve_exact($n);
            $($output_reserved)*
            if $reservation.is_err() {
                return $finish!(Err(Gfx942CompletionErrorV1::DependencyLedgerAllocation));
            }

            // The immutable retention and every pin were checked before reservation/commit.
            $($precommit)*
            let mut $i = 0;
            while $i < $n $($commit)* {
                let $occurrence = exact_occurrence($session, $epoch, $retention, $i, None)
                    .expect("event batch occurrence validated before issuance");
                $($writing)*
                let event_id = $owner.dependency_ledger.next_event_id + $i as u64;
                let replaced = $owner.dependency_ledger.events.insert(event_id, $occurrence);
                debug_assert!(replaced.is_none());
                $owner.slots[$occurrence.slot.index as usize].event_pins += 1;
                $events.push(Gfx942ComputeEventOccurrenceV1 { event_id, exact: $occurrence });
                $($written)*
                $i += 1;
            }
            $owner.dependency_ledger.next_event_id = $next;
            $finish!(Ok($events))
        })
    };
}

macro_rules! completion_record_single_event_body {
    ($syntax:ident, $owner:ident, $session:ident, $epoch:ident, $retention:ident, $index:ident) => {
        completion_record_single_event_body!(@annotated $syntax, $owner, $session, $epoch, $retention, $index,
            completion_issue_result, reservation, [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $session:ident, $epoch:ident, $retention:ident, $index:ident,
        $finish:ident, $reservation:ident, [$($start:tt)*], [$($reserved:tt)*]) => {
        $syntax!({
            $($start)*
            if let Err(error) = $owner.require_ready() {
                return $finish!(Err(error));
            }
            if let Err(error) = $owner.validate_bound($retention) {
                return $finish!(Err(error));
            }
            if let Err(error) = validate_logical_identity($session, $epoch) {
                return $finish!(Err(error));
            }
            if $owner.dependency_ledger.events.len() >= GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1 {
                return $finish!(Err(Gfx942CompletionErrorV1::EventCapacityExhausted));
            }
            let exact = match exact_occurrence($session, $epoch, $retention, $index, None) {
                Ok(exact) => exact,
                Err(error) => return $finish!(Err(error)),
            };
            let Some(next_event_id) = $owner.dependency_ledger.next_event_id.checked_add(1) else {
                return $finish!(Err(Gfx942CompletionErrorV1::EventIdentityExhausted));
            };
            let record = &$owner.slots[exact.slot.index as usize];
            let Some(next_event_pins) = record.event_pins.checked_add(1) else {
                return $finish!(Err(Gfx942CompletionErrorV1::SignalPinCountExhausted));
            };
            let $reservation = $owner.dependency_ledger.events.try_reserve(1);
            $($reserved)*
            if $reservation.is_err() {
                return $finish!(Err(Gfx942CompletionErrorV1::DependencyLedgerAllocation));
            }
            let event_id = $owner.dependency_ledger.next_event_id;
            let replaced = $owner.dependency_ledger.events.insert(event_id, exact);
            debug_assert!(replaced.is_none());
            $owner.dependency_ledger.next_event_id = next_event_id;
            $owner.slots[exact.slot.index as usize].event_pins = next_event_pins;
            $finish!(Ok(Gfx942ComputeEventOccurrenceV1 { event_id, exact }))
        })
    };
}

macro_rules! completion_record_dependency_batch_body {
    ($syntax:ident, $owner:ident, $session:ident, $epoch:ident, $retention:ident) => {
        $syntax!($owner.record_unbound_compute_event_batch($session, $epoch, $retention))
    };
}

macro_rules! completion_record_bound_dependency_batch_body {
    ($syntax:ident, $owner:ident, $session:ident, $epoch:ident, $bound:ident) => {
        $syntax!($owner.record_dependency_event_batch_v1($session, $epoch, &$bound.retention))
    };
}

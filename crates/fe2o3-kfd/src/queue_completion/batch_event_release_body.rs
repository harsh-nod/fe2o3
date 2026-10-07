// The complete event roster is validated before any owner mutation. Annotation
// hooks observe real reservation outcomes and supply proof-only loop facts.
macro_rules! completion_batch_release_result {
    ($result:expr) => {
        $result
    };
}

macro_rules! completion_release_event_batch_body {
    ($syntax:ident, $owner:ident, $events:ident) => {
        completion_release_event_batch_body!(@annotated $syntax, $owner, $events,
            completion_batch_release_result, event_ids, reservation, i, remaining,
            budget_index, index, available, pending, event, released,
            [], [], [], [], [], [], [], [], [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $events:ident, $finish:ident,
     $ids:ident, $reservation:ident, $i:ident, $remaining:ident, $j:ident,
     $index:ident, $available:ident, $pending:ident, $event:ident, $released:ident,
     [$($ids_observed:tt)*], [$($validation_inv:tt)*], [$($validation_snapshot:tt)*], [$($validated:tt)*],
     [$($legacy_done:tt)*], [$($budget_observed:tt)*], [$($budget_inv:tt)*],
     [$($budget_snapshot:tt)*], [$($entry:tt)*], [$($debited:tt)*],
     [$($precommit:tt)*], [$($commit_inv:tt)*], [$($commit_snapshot:tt)*],
     [$($committed:tt)*]) => {
        $syntax!({
            {
                if let Err(error) = $owner.require_ready() {
                    return $finish!(Err((error, $events)));
                }
                let mut $ids = HashSet::<u64>::new();
                let $reservation = $ids.try_reserve($events.len());
                $($ids_observed)*
                if $reservation.is_err() {
                    return $finish!(Err((Gfx942CompletionErrorV1::DependencyLedgerAllocation, $events)));
                }
                let mut $i = 0;
                while $i < $events.len() $($validation_inv)* {
                    $($validation_snapshot)*
                    let event = &$events[$i];
                    if !$ids.insert(event.event_id) {
                        return $finish!(Err((Gfx942CompletionErrorV1::DuplicateDependency, $events)));
                    }
                    if let Err(error) = $owner.validate_active_event(event) {
                        return $finish!(Err((error, $events)));
                    }
                    if let Err(error) = $owner.validate_live_occurrence(event.exact) {
                        return $finish!(Err((error, $events)));
                    }
                    if $owner.slots[event.exact.slot.index as usize].event_pins == 0 {
                        return $finish!(Err((Gfx942CompletionErrorV1::StaleEventOccurrence, $events)));
                    }
                    $($validated)*
                    $i += 1;
                }
                $($legacy_done)*
                if $events.len() > 1 {
                    let mut $remaining = HashMap::<u32, u32>::new();
                    let $reservation = $remaining.try_reserve($events.len());
                    $($budget_observed)*
                    if $reservation.is_err() {
                        return $finish!(Err((Gfx942CompletionErrorV1::DependencyLedgerAllocation, $events)));
                    }
                    let mut $j = 0;
                    while $j < $events.len() $($budget_inv)* {
                        $($budget_snapshot)*
                        let $index = $events[$j].exact.slot.index;
                        let pins = $owner.slots[$index as usize].event_pins;
                        let $available = $remaining.entry($index).or_insert(pins);
                        $($entry)*
                        let Some(next) = $available.checked_sub(1) else {
                            return $finish!(Err((Gfx942CompletionErrorV1::StaleEventOccurrence, $events)));
                        };
                        *$available = next;
                        $($debited)*
                        $j += 1;
                    }
                }
            }
            $($precommit)*
            let $released = $events.len();
            let mut $pending = $events.into_iter();
            loop $($commit_inv)* {
                $($commit_snapshot)*
                let Some($event) = $pending.next() else {
                    return $finish!(Ok($released));
                };
                let removed = $owner.dependency_ledger.events.remove(&$event.event_id);
                debug_assert_eq!(removed, Some($event.exact));
                $owner.slots[$event.exact.slot.index as usize].event_pins -= 1;
                $($committed)*
            }
        })
    };
}

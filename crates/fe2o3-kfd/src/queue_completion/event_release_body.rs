// Shared executable host-ledger release and validation. Native observation and
// the public session wrapper are separate obligations.
macro_rules! completion_require_ready_body {
    ($syntax:ident, $owner:ident) => {
        $syntax!({
            if $owner.phase == CompletionOwnerPhaseV1::Ready {
                Ok(())
            } else {
                Err(Gfx942CompletionErrorV1::Poisoned)
            }
        })
    };
}

macro_rules! completion_validate_active_event_body {
    ($syntax:ident, $owner:ident, $event:ident) => {
        $syntax!({
            if $owner.dependency_ledger.events.get(&$event.event_id) != Some(&$event.exact) {
                return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
            }
            Ok(())
        })
    };
}

macro_rules! completion_validate_live_occurrence_body {
    ($syntax:ident, $owner:ident, $exact:ident) => {
        $syntax!({
            if $exact.queue != $owner.queue || $exact.signal_mapping != $owner.signal_mapping {
                return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
            }
            let Some(record) = $owner.slots.get($exact.slot.index as usize) else {
                return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
            };
            if record.generation != $exact.slot.generation
                || !match record.phase {
                    CompletionSlotPhaseV1::Bound { batch_id }
                        | CompletionSlotPhaseV1::Published { batch_id }
                        | CompletionSlotPhaseV1::Completed { batch_id }
                        => batch_id == $exact.batch_id,
                    CompletionSlotPhaseV1::Available => false,
                }
            {
                return Err(Gfx942CompletionErrorV1::StaleEventOccurrence);
            }
            Ok(())
        })
    };
}

macro_rules! completion_event_release_preflight_body {
    ($syntax:ident, $owner:ident, $event:ident) => {
        $syntax!({
            $owner.require_ready()?;
            $owner.validate_active_event($event)?;
            $owner.validate_live_occurrence($event.exact)?;
            let record = &$owner.slots[$event.exact.slot.index as usize];
            record.event_pins.checked_sub(1)
                .ok_or(Gfx942CompletionErrorV1::StaleEventOccurrence)
        })
    };
}

macro_rules! completion_release_event_body {
    ($syntax:ident, $owner:ident, $event:ident) => {
        $syntax!({
            let next = match $owner.event_release_preflight(&$event) {
                Ok(next) => next,
                Err(error) => return Err((error, $event)),
            };
            let removed = $owner.dependency_ledger.events.remove(&$event.event_id);
            debug_assert_eq!(removed, Some($event.exact));
            $owner.slots[$event.exact.slot.index as usize].event_pins = next;
            Ok(Gfx942ComputeEventReleaseObservationV1)
        })
    };
}

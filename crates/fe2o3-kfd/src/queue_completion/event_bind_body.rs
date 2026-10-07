macro_rules! completion_validate_published_body {
    ($syntax:ident, $owner:ident, $retention:ident) => {
        $syntax!({
            if $retention.last_packet_id.is_none() {
                return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
            }
            $owner.validate_retention(
                $retention,
                CompletionSlotPhaseV1::Published {
                    batch_id: $retention.batch_id,
                },
            )
        })
    };
}

macro_rules! completion_mark_published_retaining_body {
    ($syntax:ident, $owner:ident, $retention:ident, $last:ident, $n:ident) => {
        completion_mark_published_retaining_body!(@annotated $syntax, $owner, $retention, $last, $n,
            i, [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $retention:ident, $last:ident, $n:ident,
        $i:ident, [$($start:tt)*], [$($invariant:tt)*], [$($step:tt)*]) => {
        $syntax!({
            if let Err(error) = $owner.validate_bound(&$retention) {
                return Err((error, $retention));
            }
            $($start)*
            let mut $i = 0;
            while $i < $n
                $($invariant)*
            {
                let slot = $retention.slots[$i];
                $owner.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Published {
                    batch_id: $retention.batch_id,
                };
                $($step)*
                $i += 1;
            }
            $retention.last_packet_id = Some($last);
            Ok(Gfx942CompletionBatchV1 { retention: $retention })
        })
    };
}

macro_rules! completion_exact_occurrence_body {
    ($syntax:ident, $session:ident, $epoch:ident, $retention:ident, $index:ident, $packet:ident) => {
        $syntax!({
            let slot = *$retention
                .slots
                .get($index)
                .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
            let dispatch = $retention
                .dispatches
                .get($index)
                .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
            Ok(ExactCompletionOccurrenceV1 {
                session_occurrence: $session,
                source_acceptance_epoch: $epoch,
                batch_id: $retention.batch_id,
                queue: $retention.queue,
                signal_mapping: $retention.signal_mapping,
                slot,
                dispatch_generation: dispatch.dispatch_generation,
                packet_id: $packet,
            })
        })
    };
}

macro_rules! completion_packet_id_at_body {
    ($syntax:ident, $retention:ident, $index:ident, $n:ident) => {
        $syntax!({
            if $index >= $n {
                return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
            }
            let packet_count = u64::try_from($n)
                .map_err(|_error| Gfx942CompletionErrorV1::StaleBatchGeneration)?;
            let batch_index = u64::try_from($index)
                .map_err(|_error| Gfx942CompletionErrorV1::StaleBatchGeneration)?;
            let last = $retention
                .last_packet_id
                .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
            let next = last
                .checked_add(1)
                .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
            let first = next
                .checked_sub(packet_count)
                .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
            first
                .checked_add(batch_index)
                .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)
        })
    };
}

macro_rules! completion_bind_event_batch_body {
    ($syntax:ident, $owner:ident, $events:ident, $batch:ident, $n:ident) => {
        completion_bind_event_batch_body!(@annotated $syntax, $owner, $events, $batch, $n,
            i, [], [], [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $events:ident, $batch:ident, $n:ident,
        $i:ident, [$($start:tt)*], [$($preflight:tt)*], [$($checking:tt)*], [$($validated:tt)*],
        [$($before_commit:tt)*], [$($commit:tt)*], [$($writing:tt)*], [$($step:tt)*]) => {
        $syntax!({
            $($start)*
            if let Err(error) = $owner.require_ready() {
                return Err((error, $events));
            }
            if let Err(error) = $owner.validate_published(&$batch.retention) {
                return Err((error, $events));
            }
            if $events.len() != $n {
                return Err((Gfx942CompletionErrorV1::StaleEventOccurrence, $events));
            }
            let mut $i = 0;
            while $i < $events.len()
                $($preflight)*
            {
                $($checking)*
                let event = &$events[$i];
                if event.exact.packet_id.is_some() {
                    return Err((Gfx942CompletionErrorV1::EventAlreadyBound, $events));
                }
                if let Err(error) = $owner.validate_active_event(event) {
                    return Err((error, $events));
                }
                let packet_id = match packet_id_at(&$batch.retention, $i) {
                    Ok(packet_id) => packet_id,
                    Err(error) => return Err((error, $events)),
                };
                let expected = match exact_occurrence(
                    event.exact.session_occurrence,
                    event.exact.source_acceptance_epoch,
                    &$batch.retention,
                    $i,
                    Some(packet_id),
                ) {
                    Ok(expected) => expected,
                    Err(error) => return Err((error, $events)),
                };
                let mut unbound = expected;
                unbound.packet_id = None;
                if event.exact != unbound {
                    return Err((Gfx942CompletionErrorV1::StaleEventOccurrence, $events));
                }
                $($validated)*
                $i += 1;
            }

            // Preflight authenticated every row and packet range before any write.
            $($before_commit)*
            let mut $i = 0;
            while $i < $events.len()
                $($commit)*
            {
                $($writing)*
                let exact = ExactCompletionOccurrenceV1 {
                    packet_id: Some(
                        packet_id_at(&$batch.retention, $i)
                            .expect("event batch packet range validated before binding"),
                    ),
                    ..$events[$i].exact
                };
                let retained = match $owner.dependency_ledger.events.entry($events[$i].event_id) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        Some(entry.into_mut())
                    }
                    std::collections::hash_map::Entry::Vacant(_entry) => None,
                };
                *retained.expect("event batch was authenticated before binding") = exact;
                $events[$i].exact = exact;
                $($step)*
                $i += 1;
            }
            Ok($events)
        })
    };
}

macro_rules! completion_bind_dependency_event_batch_body {
    ($syntax:ident, $owner:ident, $events:ident, $batch:ident) => {
        $syntax!($owner.bind_compute_event_batch_after_publication($events, $batch))
    };
}

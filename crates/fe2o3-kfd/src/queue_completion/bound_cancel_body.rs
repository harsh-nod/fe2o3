// Shared retention validation and cancellation, with proof-only loop hooks.
macro_rules! completion_packet_count_body {
    ($syntax:ident, $n:ident) => {
        $syntax!({
            if $n == 0 {
                return Err(Gfx942CompletionErrorV1::ZeroPacketCount);
            }
            if $n > COMPLETION_SIGNAL_CAPACITY_V1 {
                return Err(Gfx942CompletionErrorV1::PacketCountExceedsMaximum {
                    requested: $n,
                    maximum: COMPLETION_SIGNAL_CAPACITY_V1,
                });
            }
            Ok(())
        })
    };
}

macro_rules! completion_validate_bound_body {
    ($syntax:ident, $owner:ident, $retention:ident) => {
        $syntax!({
            if $retention.last_packet_id.is_some() {
                return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
            }
            $owner.validate_retention(
                $retention,
                CompletionSlotPhaseV1::Bound {
                    batch_id: $retention.batch_id,
                },
            )
        })
    };
}

macro_rules! completion_validate_retention_body {
    ($syntax:ident, $owner:ident, $retention:ident, $expected:ident, $n:ident) => {
        completion_validate_retention_body!(@annotated $syntax, $owner, $retention,
            $expected, $n, seen_slots, batch_index, slot, word_index, bit,
            [], [], [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $retention:ident, $expected:ident, $n:ident,
     $seen:ident, $i:ident, $slot:ident, $word:ident, $bit:ident,
     [$($initial:tt)*], [$($invariants:tt)*], [$($before:tt)*], [$($before_write:tt)*], [$($after_write:tt)*]) => {
        $syntax!({
            validate_packet_count::<$n>()?;
            if $retention.queue != $owner.queue || $retention.signal_mapping != $owner.signal_mapping {
                return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
            }
            let mut $seen = [0_u64; COMPLETION_SIGNAL_CAPACITY_V1.div_ceil(64)];
            let mut $i = 0;
            $($initial)*
            while $i < $n $($invariants)* {
                $($before)*
                let $slot = &$retention.slots[$i];
                let Some(record) = $owner.slots.get($slot.index as usize) else {
                    return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
                };
                let $word = $slot.index as usize / 64;
                let $bit = 1_u64 << ($slot.index % 64);
                if record.generation != $slot.generation
                    || record.phase != $expected
                    || $retention.dispatches[$i].queue != $retention.queue
                    || $retention.dispatches[$i].dispatch_generation == 0
                    || $retention.dispatches[$i].code.allocation.vm != $retention.queue.vm
                    || $retention.dispatches[$i].kernarg.allocation.vm != $retention.queue.vm
                    || $seen[$word] & $bit != 0
                {
                    return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
                }
                $($before_write)*
                $seen[$word] |= $bit;
                $($after_write)*
                $i += 1;
            }
            Ok(())
        })
    };
}

macro_rules! completion_require_unpinned_body {
    ($syntax:ident, $owner:ident, $slots:ident, $n:ident) => {
        completion_require_unpinned_body!(@annotated $syntax, $owner, $slots, $n, i, [])
    };
    (@annotated $syntax:ident, $owner:ident, $slots:ident, $n:ident, $i:ident,
     [$($invariants:tt)*]) => {
        $syntax!({
            let mut $i = 0;
            while $i < $n $($invariants)* {
                let slot = &$slots[$i];
                let record = &$owner.slots[slot.index as usize];
                if record.event_pins != 0 || record.native_reader_pins != 0 {
                    return Err(Gfx942CompletionErrorV1::SignalPinned {
                        slot: slot.index,
                        event_pins: record.event_pins,
                        native_reader_pins: record.native_reader_pins,
                    });
                }
                $i += 1;
            }
            Ok(())
        })
    };
}

macro_rules! completion_cancel_bound_retaining_body {
    ($syntax:ident, $owner:ident, $retention:ident, $n:ident) => {
        completion_cancel_bound_retaining_body!(@annotated $syntax, $owner, $retention,
            $n, i, [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $retention:ident, $n:ident, $i:ident,
     [$($snapshot:tt)*], [$($invariants:tt)*], [$($after_write:tt)*]) => {
        $syntax!({
            if let Err(error) = $owner.validate_bound(&$retention) {
                return Err((error, $retention));
            }
            if let Err(error) = $owner.require_unpinned(&$retention.slots) {
                return Err((error, $retention));
            }
            $($snapshot)*
            let mut $i = 0;
            while $i < $n $($invariants)* {
                let slot = &$retention.slots[$i];
                $owner.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Available;
                $($after_write)*
                $i += 1;
            }
            Ok(())
        })
    };
}

// The scan is read-only; every refusal precedes the three reservation writes.
macro_rules! dispatch_capacity_slots_body {
    ($syntax:ident, $profile:ident) => {
        $syntax!({
            match $profile {
                Self::Default64 => GFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1,
                Self::Qualification1024 => 1024,
            }
        })
    };
}

macro_rules! dispatch_preflight_reservation_body {
    ($syntax:ident, $owner:ident, $queue:ident) => {
        dispatch_preflight_reservation_body!(@annotated $syntax, $owner, $queue,
            index, has_vacant_slot, [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $queue:ident,
     $index:ident, $vacant:ident, [$($invariants:tt)*], [$($found:tt)*], [$($step:tt)*]) => {
        $syntax!({
            $owner.ensure_not_poisoned()?;
            if let Some(bound) = $owner.recipe_queue {
                if bound != $queue {
                    return Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration);
                }
            }
            let mut $vacant = false;
            let mut $index = 0;
            while $index < $owner.slots.len() $($invariants)* {
                let slot = &$owner.slots[$index];
                if slot.phase == DispatchEpochPhaseV1::Vacant {
                    $vacant = true;
                    if let Some(slot_generation) = slot.slot_generation.checked_add(1) {
                        $($found)*
                        let dispatch_generation = $owner.next_generation;
                        dispatch_generation.checked_add(1)
                            .ok_or(Gfx942DispatchBindingErrorV1::GenerationExhausted)?;
                        return Ok(($index, dispatch_generation, slot_generation));
                    }
                }
                $($step)*
                $index += 1;
            }
            if $vacant {
                Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
            } else {
                Err(Gfx942DispatchBindingErrorV1::DispatchEpochCapacity {
                    maximum: $owner.capacity_profile.slots(),
                })
            }
        })
    };
}

macro_rules! dispatch_reserve_epoch_body {
    ($syntax:ident, $owner:ident, $queue:ident, $roster:ident) => {
        $syntax!({
            let (slot_index, dispatch_generation, slot_generation) =
                $owner.preflight_reservation($queue)?;
            if $roster.queue != $queue
                || $roster.dispatch_generation != dispatch_generation
                || $roster.packet_count == 0
                || ($owner.capacity_profile == FixedDispatchCapacityProfileV1::Qualification1024
                    && $roster.packet_count != 1)
            {
                return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
            }
            let next_generation = dispatch_generation + 1;
            if slot_index > u16::MAX as usize {
                return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
            }
            let slot_index_u16 = slot_index as u16;

            $owner.recipe_queue = Some($queue);
            $owner.next_generation = next_generation;
            $owner.slots[slot_index] = DispatchEpochSlotV1 {
                slot_generation,
                phase: DispatchEpochPhaseV1::Reserved {
                    dispatch_generation,
                    expected_roster: $roster,
                },
            };
            Ok(DispatchEpochIdentityV1 {
                queue: $queue,
                recipe_occurrence: $owner.recipe_occurrence,
                slot_index: slot_index_u16,
                slot_generation,
                dispatch_generation,
            })
        })
    };
}

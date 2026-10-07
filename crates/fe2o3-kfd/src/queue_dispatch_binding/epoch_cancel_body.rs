// Shared host dispatch identity checks and reserved-epoch cancellation.
macro_rules! dispatch_not_poisoned_body {
    ($syntax:ident, $owner:ident) => {
        $syntax!({
            if $owner.poisoned {
                Err(Gfx942DispatchBindingErrorV1::Poisoned)
            } else {
                Ok(())
            }
        })
    };
}

macro_rules! dispatch_require_identity_body {
    ($syntax:ident, $owner:ident, $identity:ident, $expected:ident) => {
        $syntax!({
            $owner.ensure_not_poisoned()?;
            if $identity.recipe_occurrence != $owner.recipe_occurrence
                || Some($identity.queue) != $owner.recipe_queue
                || $identity.slot_index as usize >= $owner.slots.len()
            {
                return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
            }
            let slot = $owner.slots[$identity.slot_index as usize];
            if slot.slot_generation != $identity.slot_generation || slot.phase != $expected {
                return Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration);
            }
            Ok(())
        })
    };
}

macro_rules! dispatch_expected_roster_body {
    ($syntax:ident, $owner:ident, $identity:ident) => {
        $syntax!({
            $owner.ensure_not_poisoned()?;
            let slot = $owner
                .slots
                .get($identity.slot_index as usize)
                .ok_or(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)?;
            match slot.phase {
                DispatchEpochPhaseV1::Reserved {
                    dispatch_generation,
                    expected_roster,
                } if dispatch_generation == $identity.dispatch_generation => Ok(expected_roster),
                _ => Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration),
            }
        })
    };
}

macro_rules! dispatch_cancel_epoch_body {
    ($syntax:ident, $owner:ident, $identity:ident) => {
        $syntax!({
            $owner.require_identity(
                $identity,
                DispatchEpochPhaseV1::Reserved {
                    dispatch_generation: $identity.dispatch_generation,
                    expected_roster: $owner.expected_roster($identity)?,
                },
            )?;
            $owner.slots[$identity.slot_index as usize].phase = DispatchEpochPhaseV1::Vacant;
            Ok(())
        })
    };
}

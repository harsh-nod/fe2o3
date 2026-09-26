//! Move-only scaled epoch storage reserved before native preparation entry.

use super::*;

#[cfg(test)]
#[path = "generation_preflight/tests.rs"]
mod tests;

#[derive(Clone, Copy)]
pub(in crate::queue) enum DispatchGenerationSeedV1 {
    Fresh,
    Recycled(u64),
    Detached(u64),
    Pristine(u64),
}

impl DispatchGenerationSeedV1 {
    fn next(self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        match self {
            Self::Fresh | Self::Detached(0) => Ok(1),
            Self::Recycled(predecessor) | Self::Detached(predecessor) => {
                next_dispatch_generation_after_recycled_v1(predecessor)
            }
            Self::Pristine(next) => Ok(next),
        }
    }
}

pub(in crate::queue) struct PreparedDispatchGenerationV1(DispatchGenerationOwnerV1);

impl PreparedDispatchGenerationV1 {
    pub(in crate::queue) fn ensure_preallocated<const N: usize>(
        slot: &mut Option<Self>,
        capacity: &Gfx942FixedDispatchCapacityV1,
        seed: DispatchGenerationSeedV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        capacity.validate_batch::<N>()?;
        if let Some(prepared) = slot.as_ref() {
            prepared.validate(capacity, seed)
        } else {
            *slot = Self::preallocate::<N>(capacity, seed)?;
            Ok(())
        }
    }

    pub(in crate::queue) fn preallocate<const N: usize>(
        capacity: &Gfx942FixedDispatchCapacityV1,
        seed: DispatchGenerationSeedV1,
    ) -> Result<Option<Self>, Gfx942DispatchBindingErrorV1> {
        capacity.validate_batch::<N>()?;
        if capacity.profile == FixedDispatchCapacityProfileV1::Default64 {
            return Ok(None);
        }
        DispatchGenerationOwnerV1::with_capacity(
            seed.next()?,
            capacity.profile,
            capacity.account.as_ref(),
        )
        .map(|owner| Some(Self(owner)))
    }

    fn validate(
        &self,
        capacity: &Gfx942FixedDispatchCapacityV1,
        seed: DispatchGenerationSeedV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.0.ensure_pristine()?;
        if capacity.profile != FixedDispatchCapacityProfileV1::Qualification1024
            || !capacity.matches(self.0.capacity_profile, self.0.slots.account())
            || self.0.next_generation != seed.next()?
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        Ok(())
    }

    pub(super) fn take_for(
        prepared: &mut Option<Self>,
        capacity: &Gfx942FixedDispatchCapacityV1,
        seed: DispatchGenerationSeedV1,
    ) -> Result<DispatchGenerationOwnerV1, Gfx942DispatchBindingErrorV1> {
        if capacity.profile == FixedDispatchCapacityProfileV1::Default64 {
            if prepared.is_some() {
                return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
            }
            // Preserve the original default-profile allocation/seed timing.
            return DispatchGenerationOwnerV1::with_next_generation(seed.next()?);
        }
        prepared
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .validate(capacity, seed)?;
        Ok(prepared.take().expect("validated move-only epoch owner").0)
    }
}

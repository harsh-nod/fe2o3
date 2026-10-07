//! Original, separately accounted slot generations over one immutable backing.

use super::*;

/// Prepaid arena metadata, required before the original VM/DATA is consumed.
/// The 1024 per-slot 64-cell epoch tables, recipe table, common epoch table and
/// retained premise table debit the supplied account. Allocator overhead and
/// native allocations are outside this host-metadata payload contract.
pub struct Gfx942NativeFillArenaStorageV1 {
    recipes: HostMetadataTableV1<Option<ArenaRecipeV1>>,
    common: Option<DispatchGenerationOwnerV1>,
    pub(in crate::queue) premises: Option<Box<ArenaPremisesV1>>,
}

impl Gfx942NativeFillArenaStorageV1 {
    pub fn preallocate(
        account: ResourceCreditAccountV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        let mut recipes = HostMetadataTableV1::try_new(SLOTS, Some(&account), || None)
            .map_err(|_| rejected(0, "arena recipe table capacity"))?;
        for (index, slot) in recipes.iter_mut().enumerate() {
            *slot = Some(ArenaRecipeV1 {
                index,
                generation: DispatchGenerationOwnerV1::with_capacity(
                    1,
                    FixedDispatchCapacityProfileV1::Default64,
                    Some(&account),
                )?,
                accepted: false,
                copied_generation: None,
            });
        }
        let common = DispatchGenerationOwnerV1::with_capacity(
            1,
            FixedDispatchCapacityProfileV1::Default64,
            Some(&account),
        )?;
        let premises = Box::new(ArenaPremisesV1::preallocate(&account)?);
        Ok(Self {
            recipes,
            common: Some(common),
            premises: Some(premises),
        })
    }

    pub(in crate::queue) fn identity(&self) -> u64 {
        match self.recipes.first().and_then(Option::as_ref) {
            Some(recipe) => recipe.generation.recipe_occurrence,
            None => std::process::abort(),
        }
    }

    pub(in crate::queue) fn recipe(
        &mut self,
        index: usize,
    ) -> Result<&mut ArenaRecipeV1, Gfx942DispatchBindingErrorV1> {
        self.recipes
            .get_mut(index)
            .and_then(Option::as_mut)
            .filter(|recipe| recipe.index == index)
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
    }

    pub(in crate::queue) fn settled(&self) -> bool {
        self.recipes.len() == SLOTS
            && self.recipes.iter().enumerate().all(|(index, recipe)| {
                recipe.as_ref().is_some_and(|recipe| {
                    recipe.index == index
                        && recipe.generation.ensure_prepared().is_ok()
                        && (!recipe.accepted || recipe.generation.returned_generation().is_ok())
                })
            })
    }

    pub(in crate::queue) fn prepare(
        &mut self,
        memory: &mut impl preparation::PreparationMemoryV1,
        programs: &[ValidatedKernelEnvelope<'_>],
        custody: &mut FixedDispatchPreparationCustodyV1<SLOTS, Gfx942NativeFillArenaPacketsV1>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let common = self
            .common
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        common.ensure_pristine()?;
        if common.next_generation != 1 || !self.settled() || self.premises.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let common = self
            .common
            .take()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        custody.prepare_in_place(
            memory,
            programs,
            Ok(common),
            PersistentFixedDispatchControlStateV1::Ordinary,
        )
    }
}

pub(in crate::queue) struct ArenaRecipeV1 {
    index: usize,
    generation: DispatchGenerationOwnerV1,
    accepted: bool,
    copied_generation: Option<u64>,
}

impl ArenaRecipeV1 {
    pub(in crate::queue) fn bind<const N: usize>(
        &mut self,
        common: &DispatchResourceOwnerV1,
        queue: QueueKeyV1,
    ) -> Result<
        (
            Box<[CompletionPacketTemplateV1; N]>,
            DispatchEpochIdentityV1,
        ),
        Gfx942DispatchBindingErrorV1,
    > {
        if N != 1 || self.accepted || self.index >= SLOTS {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.generation.ensure_prepared()?;
        common
            .arena_premises_v1()?
            .selected(common, self.index, Some(queue))?;
        let (_, generation, _) = self.generation.preflight_reservation(queue)?;
        let templates = prepare_dispatch_templates_v1(
            core::slice::from_ref(&common.packets[self.index]),
            &common.code_identity,
            queue,
            generation,
        )?;
        let templates: Box<[CompletionPacketTemplateV1; N]> = templates
            .into_boxed_slice()
            .try_into()
            .map_err(|_| Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let roster = completion_template_dispatch_roster_v1(templates.as_slice())?;
        let identity = self.generation.reserve(queue, roster)?;
        Ok((templates, identity))
    }

    pub(in crate::queue) fn mark_published<const N: usize>(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation
            .mark_published(identity, completion.occurrence_v1()?)?;
        self.accepted = true;
        Ok(())
    }
    pub(in crate::queue) fn cancel(
        &mut self,
        identity: DispatchEpochIdentityV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.cancel_epoch(identity)
    }
    pub(in crate::queue) fn validate_published<const N: usize>(
        &self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation
            .validate_published(identity, completion.occurrence_v1()?)
    }
    pub(in crate::queue) fn mark_completed<const N: usize>(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletedBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation
            .complete_epoch(identity, completion.occurrence_v1()?)
    }
    pub(in crate::queue) fn validate_completed<const N: usize>(
        &self,
        identity: DispatchEpochIdentityV1,
        completion: &Gfx942CompletedBatchV1<N>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation
            .validate_completed(identity, completion.occurrence_v1()?)
    }
    pub(in crate::queue) fn recycle(
        &mut self,
        identity: DispatchEpochIdentityV1,
        completion: CompletionBatchOccurrenceV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.recycle_epoch(identity, completion)
    }
    pub(in crate::queue) fn poison(&mut self) {
        self.generation.poison();
    }

    pub(in crate::queue) fn validate_original_batch(
        &self,
        batch: &Gfx942DispatchBatchV1<1>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.validate_published(batch.identity, &batch.completion)
    }

    pub(in crate::queue) fn validate_original_completed(
        &self,
        batch: &Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.validate_completed(batch.identity, &batch.completion)
    }

    fn read_request(
        &self,
        common: &DispatchResourceOwnerV1,
        bytes: usize,
    ) -> Result<Gfx942CompletedDispatchReadRequestV1, Gfx942DispatchBindingErrorV1> {
        if !self.accepted {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let generation = self.generation.returned_generation()?;
        let range = common
            .arena_premises_v1()?
            .selected(common, self.index, None)?;
        let request =
            Gfx942CompletedDispatchReadRequestV1::new(generation, 0, range.offset, range.byte_len);
        validate_completed_read_destination(request, bytes)?;
        Ok(request)
    }

    #[cfg(test)]
    pub(in crate::queue) fn completed_range_for_test(
        &self,
        common: &DispatchResourceOwnerV1,
        bytes: usize,
    ) -> Result<(u64, u64, u64), Gfx942DispatchBindingErrorV1> {
        let request = self.read_request(common, bytes)?;
        Ok((
            request.offset,
            request.byte_len,
            request.dispatch_generation,
        ))
    }

    pub(in crate::queue) fn read_into(
        &mut self,
        common: &DispatchResourceOwnerV1,
        memory: &mut SharedGttMemorySessionV1,
        destination: &mut [u8],
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let request = self.read_request(common, destination.len())?;
        let DispatchDataAuthorityV1::HostVisible(authority) = &common.data[0] else {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        };
        // Selected's original dense partition has exactly one writable range for
        // this slot. Other slots may be live but cannot overlap this native copy.
        memory.copy_completed_dispatch_host_data_subrange_into(
            authority,
            request.offset,
            destination,
        )?;
        self.copied_generation = Some(request.dispatch_generation);
        Ok(())
    }
}

impl DispatchResourceOwnerV1 {
    pub(in crate::queue) fn require_arena_backing_v1(
        &self,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.arena_premises_v1()?.revalidate(self)
    }

    pub(super) fn arena_premises_v1(
        &self,
    ) -> Result<&ArenaPremisesV1, Gfx942DispatchBindingErrorV1> {
        let storage = self
            .conditional_fill
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        if storage.kernarg.is_some() || storage.premises.is_some() || storage.cohort.is_some() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        storage
            .arena
            .as_deref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
    }
}

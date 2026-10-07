//! Four independent, single-use recipes over one retained native backing.

use super::*;

#[cfg(test)]
#[path = "native_fill_registry/tests.rs"]
mod tests;

/// Inert admission of four original closed-fill programs, packets and outputs.
/// This supplies no compiler, currentness or execution authority. The exact
/// original inputs are returned on refusal, before native preparation.
pub struct Gfx942NativeFillRegistryInputsV1<'a> {
    pub(in crate::queue) cohort: Gfx942NativeFillCohortV1<'a, 4>,
}

impl<'a> Gfx942NativeFillRegistryInputsV1<'a> {
    // Refusal returns all original owners without a second, fallible allocation.
    #[allow(clippy::result_large_err)]
    pub fn admit(
        members: [Gfx942NativeFillCohortMemberV1<'a>; 4],
    ) -> Result<Self, Gfx942NativeFillCohortFailureV1<'a, 4>> {
        Gfx942NativeFillCohortV1::admit(members).map(|cohort| Self { cohort })
    }
}

/// Prepaid metadata for the closed four-original registry. Allocate this before
/// consuming the original VM or DATA owners. All six table payloads debit the
/// supplied account; this is not a claim of complete native-memory accounting.
///
/// Each recipe uses the existing bounded epoch implementation, but admits only
/// one accepted original invocation. The remaining epoch cells are not replay
/// capacity and cannot be selected through this interface.
pub struct Gfx942NativeFillRegistryStorageV1 {
    pub(in crate::queue) recipes: HostMetadataTableV1<RegistryRecipeV1>,
    common: Option<DispatchGenerationOwnerV1>,
}

impl Gfx942NativeFillRegistryStorageV1 {
    pub fn preallocate(
        account: ResourceCreditAccountV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        let mut owners = [const { None }; 4];
        for (index, owner) in owners.iter_mut().enumerate() {
            *owner = Some(RegistryRecipeV1 {
                index,
                generation: DispatchGenerationOwnerV1::with_capacity(
                    1,
                    FixedDispatchCapacityProfileV1::Default64,
                    Some(&account),
                )?,
                accepted: false,
            });
        }
        let mut index = 0;
        let recipes = HostMetadataTableV1::try_new(4, Some(&account), || {
            let Some(owner) = owners[index].take() else {
                std::process::abort();
            };
            index += 1;
            owner
        })
        .map_err(|_| Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
            operation: "native fill registry metadata",
        })?;
        let common = DispatchGenerationOwnerV1::with_capacity(
            1,
            FixedDispatchCapacityProfileV1::Default64,
            Some(&account),
        )?;
        Ok(Self {
            recipes,
            common: Some(common),
        })
    }

    pub(in crate::queue) fn settled(&self) -> bool {
        self.recipes.len() == 4
            && self.recipes.iter().all(|recipe| {
                recipe.generation.ensure_prepared().is_ok()
                    && (!recipe.accepted || recipe.generation.returned_generation().is_ok())
            })
    }

    pub(in crate::queue) fn identity(&self) -> u64 {
        self.recipes[0].generation.recipe_occurrence
    }

    pub(in crate::queue) fn prepare(
        &mut self,
        memory: &mut impl preparation::PreparationMemoryV1,
        programs: &[ValidatedKernelEnvelope<'_>],
        custody: &mut FixedDispatchPreparationCustodyV1<4>,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let common = self
            .common
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        common.ensure_pristine()?;
        if common.next_generation != 1 || !self.settled() {
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

pub(in crate::queue) struct RegistryRecipeV1 {
    index: usize,
    generation: DispatchGenerationOwnerV1,
    accepted: bool,
}

impl RegistryRecipeV1 {
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
        if N != 1 || self.accepted || self.index >= 4 {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.generation.ensure_prepared()?;
        common.require_registry_backing_v1()?;
        common.preflight_templates::<4>(queue)?;
        let (_, generation, _) = self.generation.preflight_reservation(queue)?;
        let packets = core::slice::from_ref(&common.packets[self.index]);
        let templates =
            prepare_dispatch_templates_v1(packets, &common.code_identity, queue, generation)?;
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

    pub(in crate::queue) fn read_into(
        &self,
        common: &DispatchResourceOwnerV1,
        memory: &mut SharedGttMemorySessionV1,
        destination: &mut [u8],
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        common.require_registry_backing_v1()?;
        self.read_request(&common.data_premises, destination.len())?;
        let DispatchDataAuthorityV1::HostVisible(authority) = &common.data[self.index] else {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        };
        memory.copy_completed_dispatch_host_data_subrange_into(authority, 0, destination)?;
        Ok(())
    }

    fn read_request(
        &self,
        premises: &[RetainedDataPremiseV1],
        destination_len: usize,
    ) -> Result<Gfx942CompletedDispatchReadRequestV1, Gfx942DispatchBindingErrorV1> {
        if !self.accepted || premises.len() != 4 || self.index >= 4 {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let generation = self.generation.returned_generation()?;
        let request = Gfx942CompletedDispatchReadRequestV1::new(
            generation,
            self.index,
            0,
            premises[self.index].layout.requested_bytes(),
        );
        validate_completed_read_request(&self.generation, premises, request)?;
        validate_completed_read_destination(request, destination_len)?;
        Ok(request)
    }
}

impl DispatchResourceOwnerV1 {
    fn require_registry_backing_v1(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.packets.len() != 4
            || self.code.len() != 4
            || self.code_identity.len() != 4
            || self.data.len() != 4
            || self.data_premises.len() != 4
            || !matches!(
                self.persistent_control,
                PersistentFixedDispatchControlStateV1::Ordinary
            )
            || self.generation.next_generation != 1
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.generation.ensure_prepared()?;
        self.conditional_fill
            .as_ref()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?
            .revalidate(self)
    }
}

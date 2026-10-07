//! Bounded independent recipes over one retained native backing.

use super::*;

#[cfg(test)]
#[path = "native_fill_registry/resident_tests.rs"]
mod resident_tests;
#[cfg(test)]
#[path = "native_fill_registry/tests.rs"]
mod tests;

#[path = "native_fill_registry/repeat2.rs"]
mod repeat2;
pub use repeat2::Gfx942NativeFillRegistryRepeat2StorageV1;

/// Inert admission of 2..16 original closed-fill programs, packets and outputs.
/// This supplies no compiler, currentness or execution authority. The exact
/// original inputs are returned on refusal, before native preparation.
pub struct Gfx942NativeFillResidentRegistryInputsV1<'a, const N: usize> {
    pub(in crate::queue) cohort: Gfx942NativeFillCohortV1<'a, N>,
}

/// Existing exact four-original profile.
pub type Gfx942NativeFillRegistryInputsV1<'a> = Gfx942NativeFillResidentRegistryInputsV1<'a, 4>;

impl<'a, const N: usize> Gfx942NativeFillResidentRegistryInputsV1<'a, N> {
    // Refusal returns all original owners without a second, fallible allocation.
    #[allow(clippy::result_large_err)]
    pub fn admit(
        members: [Gfx942NativeFillCohortMemberV1<'a>; N],
    ) -> Result<Self, Gfx942NativeFillCohortFailureV1<'a, N>> {
        Gfx942NativeFillCohortV1::admit(members).map(|cohort| Self { cohort })
    }
}

/// Prepaid metadata for the closed N-original registry. Allocate this before
/// consuming the original VM or DATA owners. All N+2 table payloads debit the
/// supplied account; this is not a claim of complete native-memory accounting.
///
/// Each recipe uses the existing bounded epoch implementation, but admits only
/// one accepted original invocation. The remaining epoch cells are not replay
/// capacity and cannot be selected through this interface. Counts outside
/// 2..16 refuse before resource-account admission; native DATA limits stay exact.
pub struct Gfx942NativeFillResidentRegistryStorageV1<const N: usize> {
    pub(in crate::queue) recipes: HostMetadataTableV1<RegistryRecipeV1>,
    common: Option<DispatchGenerationOwnerV1>,
    repeat2: bool,
}

/// Existing exact four-original metadata profile, retaining its six charges.
pub type Gfx942NativeFillRegistryStorageV1 = Gfx942NativeFillResidentRegistryStorageV1<4>;

impl<const N: usize> Gfx942NativeFillResidentRegistryStorageV1<N> {
    pub fn preallocate(
        account: ResourceCreditAccountV1,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        Self::preallocate_profile(account, false)
    }

    fn preallocate_profile(
        account: ResourceCreditAccountV1,
        repeat2: bool,
    ) -> Result<Self, Gfx942DispatchBindingErrorV1> {
        if !(2..=GFX942_MAX_FIXED_DISPATCH_DATA_V1).contains(&N) || (repeat2 && N != 4) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        let mut owners = [const { None }; N];
        for (index, owner) in owners.iter_mut().enumerate() {
            *owner = Some(RegistryRecipeV1 {
                index,
                count: N,
                generation: DispatchGenerationOwnerV1::with_capacity(
                    1,
                    FixedDispatchCapacityProfileV1::Default64,
                    Some(&account),
                )?,
                accepted: false,
                accepted_cycles: 0,
                copied_generation: None,
                repeat2,
            });
        }
        let mut index = 0;
        let recipes = HostMetadataTableV1::try_new(N, Some(&account), || {
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
            repeat2,
        })
    }

    pub(in crate::queue) fn settled(&self) -> bool {
        self.recipes.len() == N
            && self.recipes.iter().all(|recipe| {
                recipe.count == N
                    && recipe.index < N
                    && recipe.generation.ensure_prepared().is_ok()
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
        custody: &mut FixedDispatchPreparationCustodyV1<N>,
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
    count: usize,
    generation: DispatchGenerationOwnerV1,
    accepted: bool,
    accepted_cycles: u8,
    copied_generation: Option<u64>,
    repeat2: bool,
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
        if N != 1
            || self.accepted
            || !(2..=GFX942_MAX_FIXED_DISPATCH_DATA_V1).contains(&self.count)
            || self.index >= self.count
            || self.accepted_cycles >= if self.repeat2 { 2 } else { 1 }
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.generation.ensure_prepared()?;
        common.require_registry_backing_v1(self.count)?;
        common.preflight_registry_templates_v1(self.count, queue)?;
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
        self.accepted_cycles = self
            .accepted_cycles
            .checked_add(1)
            .unwrap_or_else(|| std::process::abort());
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
        &mut self,
        common: &DispatchResourceOwnerV1,
        memory: &mut SharedGttMemorySessionV1,
        destination: &mut [u8],
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        common.require_registry_backing_v1(self.count)?;
        let request = self.read_request(&common.data_premises, destination.len())?;
        let DispatchDataAuthorityV1::HostVisible(authority) = &common.data[self.index] else {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        };
        memory.copy_completed_dispatch_host_data_subrange_into(authority, 0, destination)?;
        self.copied_generation = Some(request.dispatch_generation);
        Ok(())
    }

    fn read_request(
        &self,
        premises: &[RetainedDataPremiseV1],
        destination_len: usize,
    ) -> Result<Gfx942CompletedDispatchReadRequestV1, Gfx942DispatchBindingErrorV1> {
        if !self.accepted || premises.len() != self.count || self.index >= self.count {
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
    fn require_registry_backing_v1(
        &self,
        count: usize,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if !(2..=GFX942_MAX_FIXED_DISPATCH_DATA_V1).contains(&count)
            || self.packets.len() != count
            || self.code.len() != count
            || self.code_identity.len() != count
            || self.data.len() != count
            || self.data_premises.len() != count
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

    fn preflight_registry_templates_v1(
        &self,
        count: usize,
        queue: QueueKeyV1,
    ) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        // Keep the original const-generic complete-live-set checker unchanged.
        // Count comes from the original closed recipe, never a submit argument.
        match count {
            2 => self.preflight_templates::<2>(queue),
            3 => self.preflight_templates::<3>(queue),
            4 => self.preflight_templates::<4>(queue),
            5 => self.preflight_templates::<5>(queue),
            6 => self.preflight_templates::<6>(queue),
            7 => self.preflight_templates::<7>(queue),
            8 => self.preflight_templates::<8>(queue),
            9 => self.preflight_templates::<9>(queue),
            10 => self.preflight_templates::<10>(queue),
            11 => self.preflight_templates::<11>(queue),
            12 => self.preflight_templates::<12>(queue),
            13 => self.preflight_templates::<13>(queue),
            14 => self.preflight_templates::<14>(queue),
            15 => self.preflight_templates::<15>(queue),
            16 => self.preflight_templates::<16>(queue),
            _ => Err(Gfx942DispatchBindingErrorV1::ResourcePhase),
        }
    }
}

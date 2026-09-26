//! Unpublished recipe custody. No value here denotes a completed dispatch.

use super::*;
use crate::shared_memory::ControlCleanupCustodyV1;

pub(crate) struct PristineDispatchContinuationV1 {
    next_generation: u64,
    capacity_profile: FixedDispatchCapacityProfileV1,
    account: Option<ResourceCreditAccountV1>,
}

impl PristineDispatchContinuationV1 {
    #[cfg(test)]
    pub(in crate::queue) fn from_fresh_capacity_for_test(
        capacity: &Gfx942FixedDispatchCapacityV1,
    ) -> Self {
        DispatchGenerationOwnerV1::with_capacity(1, capacity.profile, capacity.account.as_ref())
            .unwrap()
            .into_pristine_continuation()
    }

    pub(in crate::queue) fn matches_capacity(
        &self,
        capacity: &Gfx942FixedDispatchCapacityV1,
    ) -> bool {
        capacity.matches(self.capacity_profile, self.account.as_ref())
    }

    #[cfg(test)]
    pub(in crate::queue) fn next_generation_for_test(&self) -> u64 {
        self.next_generation
    }

    #[cfg(test)]
    pub(in crate::queue) fn invalidate_generation_for_test(&mut self) {
        self.next_generation = u64::MAX;
    }

    fn resume(self) -> Result<DispatchGenerationOwnerV1, Gfx942DispatchBindingErrorV1> {
        DispatchGenerationOwnerV1::with_capacity(
            self.next_generation,
            self.capacity_profile,
            self.account.as_ref(),
        )
    }
}

impl DispatchGenerationOwnerV1 {
    fn ensure_pristine(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.ensure_not_poisoned()?;
        if self.recipe_queue.is_some()
            || self.recycled_generation.is_some()
            || self
                .slots
                .iter()
                .any(|slot| *slot != DispatchEpochSlotV1::VACANT)
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        if self.next_generation == 0 || self.next_generation.checked_add(1).is_none() {
            return Err(Gfx942DispatchBindingErrorV1::GenerationExhausted);
        }
        Ok(())
    }

    fn into_pristine_continuation(self) -> PristineDispatchContinuationV1 {
        PristineDispatchContinuationV1 {
            next_generation: self.next_generation,
            capacity_profile: self.capacity_profile,
            account: self.slots.account().cloned(),
        }
    }
}

pub(crate) struct PristineAbortBuffersV1 {
    data: Vec<Gfx942FixedDispatchDataV1>,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
}

/// Remains rooted outside both control disposal and the closing model retake.
pub(crate) struct PristineDispatchAbortV1 {
    kernarg: Option<KernargAuthority>,
    code: Vec<CodeAuthority>,
    active_control: Option<ControlCleanupCustodyV1>,
    data: Vec<Gfx942FixedDispatchDataV1>,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    continuation: PristineDispatchContinuationV1,
    started: bool,
    complete: bool,
}

#[cfg(test)]
#[derive(Debug, Eq, PartialEq)]
pub(in crate::queue) struct PristineAbortSnapshotV1 {
    pub(in crate::queue) kernarg: Option<crate::shared_memory::SharedGttAllocationIdentityV1>,
    pub(in crate::queue) code: Vec<crate::shared_memory::SharedGttAllocationIdentityV1>,
    pub(in crate::queue) active: Option<crate::shared_memory::ControlCleanupObservationV1>,
    data: Vec<PristineDataSnapshotV1>,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    storage: [(usize, usize); 3],
    continuation: u64,
    started: bool,
    complete: bool,
}

#[cfg(test)]
type PristineDataSnapshotV1 = (
    Gfx942SdmaBufferStorageIdentityV1,
    Gfx942FixedDispatchDataLayoutV1,
    bool,
    Option<Gfx942DeviceContentDescriptorV1>,
);

#[cfg(test)]
impl PristineAbortSnapshotV1 {
    pub(in crate::queue) fn assert_preserved_inputs(&self, before: &Self, complete: bool) {
        assert_eq!(self.data, before.data);
        assert_eq!(self.identities, before.identities);
        assert_eq!(self.storage, before.storage);
        assert_eq!(self.continuation, before.continuation);
        assert!(self.started);
        assert_eq!(self.complete, complete);
    }
}

pub(crate) trait PristineControlReleaseV1 {
    fn release_control(
        &mut self,
        custody: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError>;
}

impl PristineControlReleaseV1 for SharedGttMemorySessionV1 {
    fn release_control(
        &mut self,
        custody: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        self.release_control_in_place_v1(custody)
    }
}

fn authority_layout(authority: &DispatchDataAuthorityV1) -> Gfx942FixedDispatchDataLayoutV1 {
    match authority {
        DispatchDataAuthorityV1::Device(authority) => {
            let layout = authority.layout();
            Gfx942FixedDispatchDataLayoutV1::device_local(
                layout.requested_bytes(),
                layout.alignment(),
            )
        }
        DispatchDataAuthorityV1::HostVisible(authority) => Gfx942FixedDispatchDataLayoutV1 {
            kind: Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
            requested_bytes: authority.layout().requested_bytes() as u64,
            alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
        },
    }
}

impl DispatchResourceOwnerV1 {
    pub(crate) fn prepare_pristine_abort_v1(
        &self,
    ) -> Result<PristineAbortBuffersV1, Gfx942DispatchBindingErrorV1> {
        self.generation.ensure_pristine()?;
        if self.persistent_control != PersistentFixedDispatchControlStateV1::Ordinary
            || self.code.is_empty()
            || self.code.len() > GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1
            || self.data.len() != self.data_premises.len()
            || self.data.len() > MAX_DISPATCH_DATA_LEASES_V1
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        for (index, (authority, premise)) in self.data.iter().zip(&self.data_premises).enumerate() {
            if authority_layout(authority) != premise.layout
                || premise.initialized_content.is_some_and(|content| {
                    !premise.fully_initialized
                        || authority.kind() != Gfx942FixedDispatchDataKindV1::DeviceLocal
                        || content.byte_len() != premise.layout.requested_bytes()
                })
            {
                return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                    index,
                    detail: "pristine abort data premise",
                });
            }
        }
        let mut buffers = PristineAbortBuffersV1 {
            data: Vec::new(),
            identities: Vec::new(),
        };
        buffers
            .data
            .try_reserve_exact(self.data.len())
            .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidData {
                index: self.data.len(),
                detail: "pristine abort data capacity",
            })?;
        buffers
            .identities
            .try_reserve_exact(self.data.len())
            .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidData {
                index: self.data.len(),
                detail: "pristine abort identity capacity",
            })?;
        Ok(buffers)
    }

    pub(crate) fn begin_pristine_abort_v1(
        self,
        mut buffers: PristineAbortBuffersV1,
    ) -> PristineDispatchAbortV1 {
        for (authority, premise) in self.data.into_iter().zip(self.data_premises) {
            let data = match (authority, premise.initialized_content) {
                (DispatchDataAuthorityV1::Device(authority), Some(content)) => {
                    let initialized =
                        Gfx942InitializedDeviceMemoryV1::from_authenticated_full_transfer(
                            authority.into_lease(),
                            content,
                        )
                        .expect("pristine preflight checked the exact content extent");
                    Gfx942FixedDispatchDataV1::initialized(initialized)
                }
                (authority, None) => {
                    dispatch_data_from_authority_v1(authority, premise.fully_initialized)
                }
                (_, Some(_)) => {
                    unreachable!("pristine preflight rejected host content descriptors")
                }
            };
            buffers.identities.push(data.storage_identity());
            buffers.data.push(data);
        }
        PristineDispatchAbortV1 {
            kernarg: Some(self.kernarg),
            code: self.code,
            active_control: None,
            data: buffers.data,
            identities: buffers.identities,
            continuation: self.generation.into_pristine_continuation(),
            started: false,
            complete: false,
        }
    }
}

impl PristineDispatchAbortV1 {
    #[cfg(test)]
    pub(in crate::queue) fn custody_snapshot_for_test(&self) -> PristineAbortSnapshotV1 {
        PristineAbortSnapshotV1 {
            kernarg: self
                .kernarg
                .as_ref()
                .map(crate::shared_memory::PreparationMemoryFixtureV1::kernarg_identity),
            code: self
                .code
                .iter()
                .map(crate::shared_memory::PreparationMemoryFixtureV1::code_identity)
                .collect(),
            active: self
                .active_control
                .as_ref()
                .map(ControlCleanupCustodyV1::observation),
            data: self
                .data
                .iter()
                .map(|d| {
                    (
                        d.sdma_storage_identity(),
                        d.layout(),
                        d.is_fully_initialized(),
                        d.initialized_content(),
                    )
                })
                .collect(),
            identities: self.identities.clone(),
            storage: [
                (self.code.as_ptr() as usize, self.code.capacity()),
                (self.data.as_ptr() as usize, self.data.capacity()),
                (
                    self.identities.as_ptr() as usize,
                    self.identities.capacity(),
                ),
            ],
            continuation: self.continuation.next_generation,
            started: self.started,
            complete: self.complete,
        }
    }

    pub(crate) fn release_controls(
        &mut self,
        memory: &mut impl PristineControlReleaseV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        if self.started {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.started = true;
        self.active_control = Some(ControlCleanupCustodyV1::kernarg(
            self.kernarg
                .take()
                .expect("unstarted abort retains kernarg")
                .into_token(),
        ));
        self.release_active_control(memory)?;
        // Every removed authority stays rooted across its borrowed cleanup callback.
        while let Some(code) = self.code.pop() {
            self.active_control = Some(ControlCleanupCustodyV1::code(code.into_token()));
            self.release_active_control(memory)?;
        }
        self.complete = true;
        Ok(())
    }

    fn release_active_control(
        &mut self,
        memory: &mut impl PristineControlReleaseV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        let active = self.active_control.as_mut().expect("rooted active control");
        memory.release_control(active)?;
        if !active.is_complete() {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase);
        }
        self.active_control = None;
        Ok(())
    }

    pub(crate) fn into_detached(
        self,
    ) -> (
        PristineDispatchContinuationV1,
        Vec<Gfx942FixedDispatchDataV1>,
        Vec<Gfx942FixedDispatchStorageIdentityV1>,
    ) {
        assert!(
            self.complete,
            "only successful control disposal permits detached custody"
        );
        (self.continuation, self.data, self.identities)
    }
}

pub(in crate::queue) fn prepare_public_fixed_dispatch_resources_after_pristine_abort_in_place_v1<
    const N: usize,
>(
    memory: &mut impl preparation::PreparationMemoryV1,
    programs: &[ValidatedKernelEnvelope<'_>],
    custody: &mut FixedDispatchPreparationCustodyV1<N>,
    continuation: PristineDispatchContinuationV1,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    custody.prepare_in_place(
        memory,
        programs,
        continuation.resume(),
        PersistentFixedDispatchControlStateV1::Ordinary,
    )
}

#[cfg(test)]
pub(in crate::queue) fn pristine_dispatch_fixture_v1(
    next_generation: u64,
) -> (
    crate::shared_memory::PristineAbortMemoryFixtureV1,
    DispatchResourceOwnerV1,
) {
    let mut memory = crate::shared_memory::PristineAbortMemoryFixtureV1::new();
    let code = vec![memory.code(), memory.code()];
    let kernarg = memory.kernarg();
    let data = vec![
        DispatchDataAuthorityV1::Device(memory.device()),
        DispatchDataAuthorityV1::Device(memory.device()),
        DispatchDataAuthorityV1::Device(memory.device()),
        DispatchDataAuthorityV1::HostVisible(memory.host()),
        DispatchDataAuthorityV1::HostVisible(memory.host()),
    ];
    let role = Gfx942DeviceContentRoleV1::new([9; 32], 2).unwrap();
    let content = Gfx942DeviceContentDescriptorV1::from_bytes(role, &[0x5a; 17]).unwrap();
    let data_premises = data
        .iter()
        .enumerate()
        .map(|(index, authority)| RetainedDataPremiseV1 {
            layout: authority_layout(authority),
            role_identity: [0; 32],
            valid_bytes: 17,
            effect: None,
            initialized_content: (index == 2).then_some(content),
            fully_initialized: matches!(index, 1 | 2 | 4),
            writable_ranges: Box::new([]),
            completed_snapshots: Box::new([]),
        })
        .collect();
    let owner = DispatchResourceOwnerV1 {
        code,
        code_identity: Vec::new(),
        kernarg,
        packets: Vec::new(),
        data,
        data_premises,
        generation: DispatchGenerationOwnerV1::with_next_generation(next_generation).unwrap(),
        persistent_control: PersistentFixedDispatchControlStateV1::Ordinary,
    };
    (memory, owner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pristine_continuation_preserves_exact_next_generation_and_changes_occurrence() {
        for next in [1, 7, 8, u64::MAX - 1] {
            let mut owner = DispatchGenerationOwnerV1::with_next_generation(next).unwrap();
            for _ in 0..3 {
                owner.ensure_pristine().unwrap();
                let occurrence = owner.recipe_occurrence;
                owner = owner.into_pristine_continuation().resume().unwrap();
                assert_eq!(owner.next_generation, next);
                assert_ne!(owner.recipe_occurrence, occurrence);
                assert!(matches!(
                    owner.returned_generation(),
                    Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
                ));
            }
        }
    }

    #[test]
    fn pristine_rejection_checks_every_slot_and_does_not_mutate() {
        let fresh = DispatchGenerationOwnerV1::new().unwrap();
        for index in 0..GFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1 {
            for phase in [
                DispatchEpochPhaseV1::Vacant,
                DispatchEpochPhaseV1::Reserved {
                    dispatch_generation: 1,
                    expected_roster: test_completion_roster_v1(1),
                },
            ] {
                let mut owner = fresh.clone();
                owner.slots[index] = DispatchEpochSlotV1 {
                    slot_generation: 1,
                    phase,
                };
                let before = owner.clone();
                assert!(owner.ensure_pristine().is_err());
                assert_eq!(owner, before);
            }
        }
        for next in [0, u64::MAX] {
            let mut owner = fresh.clone();
            owner.next_generation = next;
            assert!(matches!(
                owner.ensure_pristine(),
                Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
            ));
        }
        let mut owner = fresh.clone();
        owner.recipe_queue = Some(test_dispatch_queue_v1());
        assert!(owner.ensure_pristine().is_err());
        owner = fresh.clone();
        owner.recycled_generation = Some(1);
        assert!(owner.ensure_pristine().is_err());
        owner = fresh;
        owner.poison();
        assert!(matches!(
            owner.ensure_pristine(),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
    }

    #[test]
    fn scaled_pristine_continuation_preserves_capacity_account_and_exact_generation() {
        use fe2o3_resource_accounting::{
            ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
        };
        let bytes = host_metadata_table_payload_bytes_v1::<DispatchEpochSlotV1>(1024).unwrap();
        let account = ResourceCreditAccountV1::new(
            ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
            1,
        )
        .unwrap();
        let mut owner = DispatchGenerationOwnerV1::with_capacity(
            7,
            FixedDispatchCapacityProfileV1::Qualification1024,
            Some(&account),
        )
        .unwrap();
        for _ in 0..3 {
            owner.ensure_pristine().unwrap();
            let occurrence = owner.recipe_occurrence;
            let continuation = owner.into_pristine_continuation();
            assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
            owner = continuation.resume().unwrap();
            assert_eq!(
                owner.capacity_profile,
                FixedDispatchCapacityProfileV1::Qualification1024
            );
            assert_eq!(owner.slots.len(), 1024);
            assert_eq!(owner.next_generation, 7);
            assert_ne!(owner.recipe_occurrence, occurrence);
            assert_eq!(
                account
                    .usage()
                    .used
                    .get(ResourceKindV1::ControlResidentBytes),
                bytes
            );
            assert_eq!(account.usage().retained_records, 1);
        }
        for generation in 7..1031 {
            owner
                .reserve(
                    test_dispatch_queue_v1(),
                    test_completion_roster_v1(generation),
                )
                .unwrap();
        }
        assert!(matches!(
            owner.reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1031)),
            Err(Gfx942DispatchBindingErrorV1::DispatchEpochCapacity { maximum: 1024 })
        ));
        drop(owner);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    }

    #[test]
    fn pristine_rejects_actual_cancelled_and_completed_epoch_histories() {
        let mut owner = DispatchGenerationOwnerV1::new().unwrap();
        let epoch = owner
            .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(1))
            .unwrap();
        assert!(owner.ensure_pristine().is_err());
        let mut cancelled = owner.clone();
        cancelled.cancel_epoch(epoch).unwrap();
        cancelled.ensure_prepared().unwrap();
        assert!(cancelled.ensure_pristine().is_err());
        let completion = test_completion_occurrence_v1(1);
        owner.mark_published(epoch, completion).unwrap();
        assert!(owner.ensure_pristine().is_err());
        owner.complete_epoch(epoch, completion).unwrap();
        assert!(owner.ensure_pristine().is_err());
        owner.recycle_epoch(epoch, completion).unwrap();
        owner.ensure_prepared().unwrap();
        assert!(owner.ensure_pristine().is_err());
    }

    #[test]
    fn pristine_abort_releases_only_control_and_preserves_all_storage_variants() {
        let (mut memory, owner) = pristine_dispatch_fixture_v1(8);
        let usage = memory.usage();
        let content = owner.data_premises[2].initialized_content.unwrap();
        let device_ids: Vec<_> = owner
            .data
            .iter()
            .filter_map(|authority| match authority {
                DispatchDataAuthorityV1::Device(authority) => Some(authority.storage_identity()),
                _ => None,
            })
            .collect();
        let buffers = owner.prepare_pristine_abort_v1().unwrap();
        let mut abort = owner.begin_pristine_abort_v1(buffers);
        let identities = abort.identities.clone();
        abort.release_controls(&mut memory).unwrap();
        assert_eq!(memory.freed(), 3);
        assert!(memory.data_is_retained());
        assert_eq!(memory.usage(), usage);
        let calls = memory.native_calls();
        assert!(abort.release_controls(&mut memory).is_err());
        assert_eq!(memory.native_calls(), calls);
        let (continuation, data, returned_ids) = abort.into_detached();
        assert_eq!(continuation.next_generation, 8);
        assert_eq!(returned_ids, identities);
        assert!(matches!(
            data[0].storage,
            DispatchDataStorageV1::Uninitialized(_)
        ));
        assert!(matches!(
            data[1].storage,
            DispatchDataStorageV1::InitializedAfterDispatch(_)
        ));
        let DispatchDataStorageV1::InitializedContent(ref initialized) = data[2].storage else {
            panic!("lost content descriptor")
        };
        assert_eq!(initialized.content(), content);
        assert!(matches!(
            data[3].storage,
            DispatchDataStorageV1::HostVisibleUninitialized(_)
        ));
        assert!(matches!(
            data[4].storage,
            DispatchDataStorageV1::HostVisibleInitialized(_)
        ));
        for (index, identity) in device_ids.into_iter().enumerate() {
            assert_eq!(
                data[index].sdma_storage_identity(),
                Gfx942SdmaBufferStorageIdentityV1::Device(identity)
            );
        }
    }

    #[test]
    fn dispatch_retention_after_pristine_abort_preserves_all_five_input_variants() {
        let (mut memory, owner) = pristine_dispatch_fixture_v1(8);
        let buffers = owner.prepare_pristine_abort_v1().unwrap();
        let mut abort = owner.begin_pristine_abort_v1(buffers);
        abort.release_controls(&mut memory).unwrap();
        let (_, mut data, _) = abort.into_detached();
        let expected: Vec<_> = data
            .iter()
            .map(|input| {
                (
                    input.sdma_storage_identity(),
                    input.layout(),
                    input.is_fully_initialized(),
                    input.initialized_content(),
                )
            })
            .collect();
        assert!(matches!(
            data[1].storage,
            DispatchDataStorageV1::InitializedAfterDispatch(_)
        ));
        let calls = memory.native_calls();
        let usage = memory.usage();
        let retained = memory.retain_data(&mut data).unwrap();
        assert!(data.is_empty());
        assert_eq!(retained.len(), 5);
        assert!(retained[1].fully_initialized);
        assert!(retained[1].initialized_content.is_none());
        for (input, (identity, layout, initialized, content)) in retained.into_iter().zip(expected)
        {
            assert_eq!(input.layout, layout);
            assert_eq!(input.fully_initialized, initialized);
            assert_eq!(input.initialized_content, content);
            let actual = match input.authority {
                DispatchDataAuthorityV1::Device(authority) => {
                    Gfx942SdmaBufferStorageIdentityV1::Device(authority.storage_identity())
                }
                DispatchDataAuthorityV1::HostVisible(authority) => {
                    Gfx942SdmaBufferStorageIdentityV1::Host(
                        authority.into_token().storage_identity(),
                    )
                }
            };
            assert_eq!(actual, identity);
        }
        assert_eq!(memory.native_calls(), calls);
        assert_eq!(memory.usage(), usage);
        assert!(memory.data_is_retained());
    }

    #[test]
    fn pristine_abort_rejects_malformed_premises_before_native_effects() {
        for mutation in 0..6 {
            let (memory, mut owner) = pristine_dispatch_fixture_v1(1);
            let calls = memory.native_calls();
            match mutation {
                0 => owner.data_premises[2].fully_initialized = false,
                1 => {
                    owner.data_premises[3].initialized_content =
                        owner.data_premises[2].initialized_content
                }
                2 => owner.data_premises[2].layout.requested_bytes += 1,
                3 => {
                    owner.data_premises.pop();
                }
                5 => {
                    let role = owner.data_premises[2].initialized_content.unwrap().role();
                    owner.data_premises[2].initialized_content =
                        Some(Gfx942DeviceContentDescriptorV1::from_bytes(role, &[0; 18]).unwrap());
                }
                _ => {
                    let DispatchDataAuthorityV1::Device(ref data) = owner.data[0] else {
                        unreachable!()
                    };
                    owner.persistent_control = PersistentFixedDispatchControlStateV1::Attached(
                        BoundedPersistentFixedDispatchControlIdentityV1::from_single(
                            PersistentFixedDispatchControlIdentityV1 {
                                queue: test_dispatch_queue_v1(),
                                semantic_sha256: [1; 32],
                                content_role: Gfx942DeviceContentRoleV1::new([1; 32], 0).unwrap(),
                                data_layout: owner.data_premises[0].layout,
                                data_storage: Gfx942SdmaBufferStorageIdentityV1::Device(
                                    data.storage_identity(),
                                ),
                                effect: DeviceDataEffectV1::ReadOnly,
                            },
                        ),
                    );
                }
            }
            assert!(owner.prepare_pristine_abort_v1().is_err());
            assert_eq!(owner.data.len(), 5);
            assert_eq!(memory.native_calls(), calls);
            assert!(memory.data_is_retained());
        }
    }

    #[test]
    fn pristine_abort_incomplete_callback_keeps_active_control_and_rejects_retry() {
        struct IncompleteCleanup(usize);
        impl PristineControlReleaseV1 for IncompleteCleanup {
            fn release_control(
                &mut self,
                _: &mut ControlCleanupCustodyV1,
            ) -> Result<(), MemorySessionError> {
                self.0 += 1;
                Ok(())
            }
        }
        let (memory, owner) = pristine_dispatch_fixture_v1(8);
        let buffers = owner.prepare_pristine_abort_v1().unwrap();
        let mut abort = owner.begin_pristine_abort_v1(buffers);
        let before = abort.custody_snapshot_for_test();
        let calls = memory.native_calls();
        let currentness = memory.currentness_calls();
        let usage = memory.usage();
        let mut cleanup = IncompleteCleanup(0);
        assert!(matches!(
            abort.release_controls(&mut cleanup),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        let after = abort.custody_snapshot_for_test();
        let active = after
            .active
            .as_ref()
            .expect("incomplete callback retains control");
        assert_eq!(active.identity, before.kernarg.unwrap());
        assert_eq!(active.owner, "Mapped");
        assert!(!active.started && !active.failed);
        assert_eq!(active.unmap, (false, None, None));
        assert_eq!(active.disposal, [(false, None); 3]);
        assert!(!active.native_disposed);
        assert_eq!(after.kernarg, None);
        assert_eq!(after.code, before.code);
        after.assert_preserved_inputs(&before, false);
        assert!(matches!(
            abort.release_controls(&mut cleanup),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        assert_eq!(cleanup.0, 1, "incomplete cleanup cannot be entered again");
        assert_eq!(abort.custody_snapshot_for_test(), after);
        assert_eq!(memory.native_calls(), calls);
        assert_eq!(memory.cleanup_call_count(), 0);
        assert_eq!(memory.currentness_calls(), currentness);
        assert_eq!(memory.usage(), usage);
        assert!(memory.data_is_retained());
    }

    #[test]
    fn pristine_abort_native_errors_and_panics_keep_data_and_disallow_retry() {
        for ordinal in 1..=3 {
            for operation in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"] {
                for panic in [false, true] {
                    let (mut memory, owner) = pristine_dispatch_fixture_v1(8);
                    let usage = memory.usage();
                    let buffers = owner.prepare_pristine_abort_v1().unwrap();
                    let mut abort = owner.begin_pristine_abort_v1(buffers);
                    let identities = abort.identities.clone();
                    let before = abort.custody_snapshot_for_test();
                    memory.fail_control(ordinal, operation, panic);
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        abort.release_controls(&mut memory)
                    }));
                    if panic {
                        assert!(result.is_err(), "{operation}");
                    } else {
                        assert!(result.unwrap().is_err(), "{operation}");
                    }
                    assert!(!abort.complete);
                    let after = abort.custody_snapshot_for_test();
                    let active = after
                        .active
                        .as_ref()
                        .expect("interrupted control remains rooted");
                    assert_eq!(
                        active.identity,
                        if ordinal == 1 {
                            before.kernarg.unwrap()
                        } else {
                            before.code[3 - ordinal]
                        }
                    );
                    assert_eq!(
                        active.owner,
                        if operation == "unmap_gpu" {
                            "Mapped"
                        } else {
                            "Unmapped"
                        }
                    );
                    assert!(active.started && active.failed);
                    assert!(memory.is_quarantined());
                    assert_eq!(after.kernarg, None);
                    assert_eq!(after.code, before.code[..3 - ordinal]);
                    assert_eq!(after.data, before.data);
                    assert_eq!(after.storage, before.storage);
                    assert_eq!(after.continuation, before.continuation);
                    assert_eq!(memory.disposed_controls(), ordinal - 1);
                    assert_eq!(abort.identities, identities);
                    assert_eq!(abort.data.len(), 5);
                    assert_eq!(memory.usage(), usage);
                    assert!(memory.data_is_retained());
                    let calls = memory.native_calls();
                    let cleanup_calls = memory.cleanup_call_count();
                    let currentness = memory.currentness_calls();
                    assert!(abort.release_controls(&mut memory).is_err());
                    assert_eq!(memory.native_calls(), calls);
                    assert_eq!(memory.cleanup_call_count(), cleanup_calls);
                    assert_eq!(memory.currentness_calls(), currentness);
                    assert_eq!(abort.custody_snapshot_for_test(), after);
                }
            }
        }
    }

    #[test]
    fn pristine_abort_rejects_partial_and_malformed_gpu_unmap_for_each_control() {
        for ordinal in 1..=3 {
            for (progress, errno) in [(0, false), (2, false), (0, true), (1, true)] {
                let (mut memory, owner) = pristine_dispatch_fixture_v1(8);
                let usage = memory.usage();
                let buffers = owner.prepare_pristine_abort_v1().unwrap();
                let mut abort = owner.begin_pristine_abort_v1(buffers);
                memory.unmap_control(ordinal, progress, errno);
                assert!(abort.release_controls(&mut memory).is_err());
                assert!(!abort.complete);
                assert_eq!(memory.disposed_controls(), ordinal - 1);
                assert_eq!(memory.usage(), usage);
                assert!(memory.data_is_retained());
                let calls = memory.native_calls();
                assert!(abort.release_controls(&mut memory).is_err());
                assert_eq!(memory.native_calls(), calls);
            }
        }
    }

    #[test]
    fn pristine_abort_sweeps_currentness_and_partial_control_cleanup() {
        let (mut memory, owner) = pristine_dispatch_fixture_v1(8);
        let buffers = owner.prepare_pristine_abort_v1().unwrap();
        let mut abort = owner.begin_pristine_abort_v1(buffers);
        let before = memory.currentness_calls();
        abort.release_controls(&mut memory).unwrap();
        let boundaries = memory.currentness_calls() - before;
        assert_eq!(boundaries, 18);
        let mut partial = false;
        for offset in 1..=boundaries {
            for panic in [false, true] {
                let (mut memory, owner) = pristine_dispatch_fixture_v1(8);
                let usage = memory.usage();
                let buffers = owner.prepare_pristine_abort_v1().unwrap();
                let mut abort = owner.begin_pristine_abort_v1(buffers);
                memory.fail_currentness(offset, panic);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    abort.release_controls(&mut memory)
                }));
                if panic {
                    assert!(result.is_err(), "boundary {offset}");
                } else {
                    assert!(result.unwrap().is_err(), "boundary {offset}");
                }
                partial |= memory.disposed_controls() > 0 && memory.disposed_controls() < 3;
                assert!(!abort.complete);
                assert_eq!(abort.data.len(), 5);
                assert_eq!(memory.usage(), usage);
                assert!(memory.data_is_retained());
                let calls = memory.native_calls();
                assert!(abort.release_controls(&mut memory).is_err());
                assert_eq!(memory.native_calls(), calls);
            }
        }
        assert!(
            partial,
            "sweep must reach later controls after confirmed disposal"
        );
    }
}

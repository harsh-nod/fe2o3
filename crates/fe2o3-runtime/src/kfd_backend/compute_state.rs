use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NativeDirtyExtentV1 {
    pub(super) compute_lane: usize,
    pub(super) data_index: usize,
    pub(super) allocation_offset: usize,
    pub(super) data_offset: u64,
    pub(super) byte_len: u64,
}

pub(super) struct ModuleRecordV1 {
    pub(super) device: u64,
    pub(super) validated: OwnedValidatedEnvelope,
    pub(super) image_sha256: [u8; 32],
}

pub(super) struct KernelRecordV1 {
    pub(super) module: u64,
    pub(super) validated: OwnedValidatedKernelEnvelope,
    pub(super) signature: [u8; 32],
}

impl fmt::Debug for ModuleRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModuleRecordV1")
            .field("device", &self.device)
            .field("image_bytes", &self.validated.bytes().len())
            .field("image_sha256", &self.image_sha256)
            .finish()
    }
}

impl fmt::Debug for KernelRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KernelRecordV1")
            .field("module", &self.module)
            .field("name", &self.validated.selected_kernel().name())
            .field("signature", &self.signature)
            .finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WritebackV1 {
    pub(super) allocation: u64,
    pub(super) allocation_offset: usize,
    pub(super) data_index: usize,
    pub(super) data_offset: u64,
    pub(super) byte_len: u64,
}

pub(super) struct ActiveSubmissionV1 {
    pub(super) id: u64,
    pub(super) stream: u64,
    pub(super) ordered_predecessor: Option<u64>,
    pub(super) deferred_ordered_predecessor_retain: bool,
    pub(super) kernel: u64,
    pub(super) dependency_depth: usize,
    pub(super) allocations: HashSet<u64>,
    pub(super) writebacks: Vec<WritebackV1>,
    pub(super) resident_descriptors: Vec<ResidentDataDescriptorV1>,
    pub(super) ordinary_recipe: Option<Arc<OwnedComputeLaunchV1>>,
    pub(super) dispatch_shape_sha256: [u8; 32],
    pub(super) published_at: Instant,
    pub(super) performance: KfdRuntimeLaunchPerformanceV1,
    pub(super) execution: Option<ActiveComputeExecutionV1>,
}

pub(super) struct PersistentPublicationProfileV1 {
    pub(super) launch: KfdProfileLaunchV1,
    pub(super) semantic_contract: Option<KfdProfileSemanticContractV1>,
    pub(super) bindings: Option<Result<Vec<KfdProfileBindingV1>, ()>>,
}

// The large test-only scripted owner keeps failure-path custody inline so the
// tests exercise the same allocation-free terminal-recovery invariant.
#[allow(clippy::large_enum_variant)]
pub(super) enum ActiveComputeExecutionV1 {
    MaterializedPrepared {
        profile: PersistentPublicationProfileV1,
    },
    Materialized(Gfx942DispatchBatchV1<1>),
    MaterializedCompleted(fe2o3_kfd::Gfx942CompletedDispatchBatchV1<1>),
    PersistentPrepared {
        allocation: u64,
        access: RuntimeAccessV1,
        prepared: Gfx942PreparedPersistentComputeDispatchV1,
        profile: PersistentPublicationProfileV1,
    },
    Persistent {
        allocation: u64,
        access: RuntimeAccessV1,
        dispatch: Gfx942PersistentComputeDispatchV1,
    },
    ThreeBindingPersistentPrepared {
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
        promotions: [Option<KfdRuntimeReadyPromotionPerformanceV1>; 3],
        restore_shells: [ThreeBindingPersistentRestoreShellV1; 3],
        prepared: Gfx942PreparedThreeBindingPersistentComputeDispatchV1,
        profile: PersistentPublicationProfileV1,
    },
    ThreeBindingPersistent {
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
        restore_shells: [ThreeBindingPersistentRestoreShellV1; 3],
        dispatch: Gfx942ThreeBindingPersistentComputeDispatchV1,
    },
    #[cfg(test)]
    ScriptedPersistent {
        allocation: u64,
        access: RuntimeAccessV1,
        device: Box<DirectionalSdmaDeviceOwnerV1>,
    },
    #[cfg(test)]
    ScriptedPersistentPrepared {
        allocation: u64,
        access: RuntimeAccessV1,
        input: Box<KfdRuntimePersistentComputeInputV1>,
        profile: PersistentPublicationProfileV1,
    },
    #[cfg(test)]
    ScriptedThreeBindingPersistent {
        admissions: [PersistentFullRangeComputeAdmissionV1; 3],
        restore_shells: [ThreeBindingPersistentRestoreShellV1; 3],
        devices: [DirectionalSdmaDeviceOwnerV1; 3],
    },
    #[cfg(test)]
    ScriptedMaterialized,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScriptedPersistentTransitionFailureV1 {
    Poll,
    Recycle,
    Detach,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OwnedComputeLaunchV1 {
    pub(super) stream: u64,
    pub(super) kernel: u64,
    pub(super) explicit_kernarg: Box<[u8]>,
    pub(super) bindings: Box<[BackendBindingV1]>,
    pub(super) geometry: crate::RuntimeLaunchGeometryV1,
    pub(super) semantic_launch: KfdRuntimeSemanticLaunchV1,
}

impl OwnedComputeLaunchV1 {
    pub(super) fn borrowed(&self) -> BackendLaunchV1<'_> {
        BackendLaunchV1 {
            stream: self.stream,
            kernel: self.kernel,
            explicit_kernarg: &self.explicit_kernarg,
            bindings: &self.bindings,
            dependencies: &[],
            geometry: self.geometry,
            semantic_launch: self.semantic_launch,
        }
    }
}

#[derive(Debug)]
pub(super) struct PendingComputeSubmissionV1 {
    pub(super) id: u64,
    pub(super) module: u64,
    pub(super) launch: Arc<OwnedComputeLaunchV1>,
    pub(super) retained_allocations: Box<[u64]>,
    pub(super) ordered_predecessor: Option<u64>,
    pub(super) explicit_success_dependencies: Box<[u64]>,
    pub(super) explicit_dependency_cursor: usize,
    pub(super) dependency_depth: usize,
}

pub(super) const RUNTIME_COMPUTE_PIPELINE_CAPACITY_V1: usize =
    fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_INFLIGHT_V1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RuntimeComputePipelineIdentityV1 {
    slot: u8,
    slot_generation: u64,
    logical_epoch: u64,
    submission: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeComputePipelinePhaseV1 {
    Published,
    Completed,
    PhysicallyRetired,
    Quarantined,
}

pub(super) struct RuntimeComputePipelineEntryV1 {
    pub(super) identity: RuntimeComputePipelineIdentityV1,
    pub(super) phase: RuntimeComputePipelinePhaseV1,
    pub(super) active: ActiveSubmissionV1,
}

struct RuntimeComputePipelineSlotV1 {
    generation: u64,
    entry: Option<RuntimeComputePipelineEntryV1>,
}

pub(super) struct RuntimeComputePipelineV1 {
    slots: Box<[RuntimeComputePipelineSlotV1; RUNTIME_COMPUTE_PIPELINE_CAPACITY_V1]>,
    live: usize,
    next_logical_epoch: Option<u64>,
    commit_frontier: Option<u64>,
}

impl RuntimeComputePipelineV1 {
    pub(super) fn vacant() -> Self {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(RUNTIME_COMPUTE_PIPELINE_CAPACITY_V1)
            .expect("fixed runtime compute pipeline allocation");
        slots.extend((0..RUNTIME_COMPUTE_PIPELINE_CAPACITY_V1).map(|_| {
            RuntimeComputePipelineSlotV1 {
                generation: 0,
                entry: None,
            }
        }));
        let slots = slots
            .into_boxed_slice()
            .try_into()
            .unwrap_or_else(|_| unreachable!("fixed runtime compute pipeline length"));
        Self {
            slots,
            live: 0,
            next_logical_epoch: Some(1),
            commit_frontier: None,
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.live
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.live == 0
    }

    pub(super) fn has_successor_capacity(&self) -> bool {
        // The logical frontier is retained separately in the lane's `active` slot.
        self.live < RUNTIME_COMPUTE_PIPELINE_CAPACITY_V1 - 1
            && self.next_logical_epoch.is_some()
            && self.slots.iter().any(|slot| {
                slot.entry.is_none()
                    && slot
                        .generation
                        .checked_add(1)
                        .is_some_and(|generation| generation != 0)
            })
    }

    // The rejected linear owner stays inline so roster insertion cannot add a
    // recovery-path allocation or lose custody on allocator failure.
    #[allow(clippy::result_large_err)]
    pub(super) fn insert_published(
        &mut self,
        active: ActiveSubmissionV1,
    ) -> Result<RuntimeComputePipelineIdentityV1, ActiveSubmissionV1> {
        if !self.has_successor_capacity() || self.contains(active.id) {
            return Err(active);
        }
        let logical_epoch = self
            .next_logical_epoch
            .expect("successor capacity checked the logical epoch");
        let Some((slot_index, slot_generation)) =
            self.slots.iter().enumerate().find_map(|(index, slot)| {
                if slot.entry.is_some() {
                    return None;
                }
                slot.generation
                    .checked_add(1)
                    .filter(|generation| *generation != 0)
                    .map(|generation| (index, generation))
            })
        else {
            return Err(active);
        };
        let identity = RuntimeComputePipelineIdentityV1 {
            slot: u8::try_from(slot_index).expect("runtime compute pipeline has 64 slots"),
            slot_generation,
            logical_epoch,
            submission: active.id,
        };
        let slot = &mut self.slots[slot_index];
        slot.generation = slot_generation;
        slot.entry = Some(RuntimeComputePipelineEntryV1 {
            identity,
            phase: RuntimeComputePipelinePhaseV1::Published,
            active,
        });
        self.live += 1;
        self.next_logical_epoch = logical_epoch.checked_add(1);
        if self.commit_frontier.is_none() {
            self.commit_frontier = Some(logical_epoch);
        }
        Ok(identity)
    }

    pub(super) fn contains(&self, submission: u64) -> bool {
        self.get(submission).is_some()
    }

    pub(super) fn get(&self, submission: u64) -> Option<&ActiveSubmissionV1> {
        self.slots.iter().find_map(|slot| {
            slot.entry
                .as_ref()
                .filter(|entry| entry.active.id == submission)
                .map(|entry| &entry.active)
        })
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &ActiveSubmissionV1> {
        self.slots
            .iter()
            .filter_map(|slot| slot.entry.as_ref().map(|entry| &entry.active))
    }

    pub(super) fn phase(&self, submission: u64) -> Option<RuntimeComputePipelinePhaseV1> {
        self.slots.iter().find_map(|slot| {
            slot.entry
                .as_ref()
                .filter(|entry| entry.active.id == submission)
                .map(|entry| entry.phase)
        })
    }

    pub(super) fn quarantine_all(&mut self) {
        for entry in self.slots.iter_mut().filter_map(|slot| slot.entry.as_mut()) {
            entry.phase = RuntimeComputePipelinePhaseV1::Quarantined;
        }
    }

    pub(super) fn take_physical_owner(
        &mut self,
        submission: u64,
    ) -> Option<(RuntimeComputePipelineIdentityV1, ActiveSubmissionV1)> {
        let slot = self.slots.iter_mut().find(|slot| {
            slot.entry.as_ref().is_some_and(|entry| {
                entry.active.id == submission
                    && matches!(
                        entry.phase,
                        RuntimeComputePipelinePhaseV1::Published
                            | RuntimeComputePipelinePhaseV1::Completed
                    )
            })
        })?;
        let entry = slot.entry.take().expect("selected pipeline entry");
        Some((entry.identity, entry.active))
    }

    // Restoration returns the exact inline owner to the caller on identity
    // mismatch; boxing it would make terminal custody recovery allocate.
    #[allow(clippy::result_large_err)]
    pub(super) fn restore(
        &mut self,
        identity: RuntimeComputePipelineIdentityV1,
        phase: RuntimeComputePipelinePhaseV1,
        active: ActiveSubmissionV1,
    ) -> Result<(), ActiveSubmissionV1> {
        let Some(slot) = self.slots.get_mut(identity.slot as usize) else {
            return Err(active);
        };
        if slot.generation != identity.slot_generation
            || slot.entry.is_some()
            || active.id != identity.submission
        {
            return Err(active);
        }
        slot.entry = Some(RuntimeComputePipelineEntryV1 {
            identity,
            phase,
            active,
        });
        Ok(())
    }

    pub(super) fn take_commit_frontier(
        &mut self,
    ) -> Option<(RuntimeComputePipelinePhaseV1, ActiveSubmissionV1)> {
        let frontier = self.commit_frontier?;
        let slot = self.slots.iter_mut().find(|slot| {
            slot.entry
                .as_ref()
                .is_some_and(|entry| entry.identity.logical_epoch == frontier)
        })?;
        let entry = slot.entry.take().expect("selected commit-frontier entry");
        self.live = self.live.checked_sub(1).expect("pipeline entry was live");
        self.commit_frontier = if self.live == 0 {
            None
        } else {
            frontier.checked_add(1)
        };
        Some((entry.phase, entry.active))
    }

    #[cfg(test)]
    pub(super) fn identity_for_submission_v1(
        &self,
        submission: u64,
    ) -> Option<RuntimeComputePipelineIdentityV1> {
        self.slots.iter().find_map(|slot| {
            slot.entry
                .as_ref()
                .filter(|entry| entry.active.id == submission)
                .map(|entry| entry.identity)
        })
    }

    #[cfg(test)]
    pub(super) fn exhaust_vacant_identities_for_test_v1(&mut self) {
        assert!(self.is_empty());
        for slot in self.slots.iter_mut() {
            slot.generation = u64::MAX;
        }
    }

    #[cfg(test)]
    pub(super) fn exhaust_logical_epochs_for_test_v1(&mut self) {
        assert!(self.is_empty());
        self.next_logical_epoch = None;
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PersistentComputeReadyFactsV1 {
    pub(super) logical_bytes: u64,
    pub(super) physical_bytes: u64,
    pub(super) authenticated_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PersistentFullRangeComputeAdmissionV1 {
    pub(super) allocation: u64,
    pub(super) access: RuntimeAccessV1,
    pub(super) source: PersistentFullRangeComputeSourceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PersistentFullRangeComputeSourceV1 {
    AuthenticatedH2d,
    RetainedControlReplay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ThreeBindingPersistentComputeAdmissionV1 {
    pub(super) bindings: [PersistentFullRangeComputeAdmissionV1; 3],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RetainedPersistentDispatchV1 {
    pub(super) allocation: u64,
    pub(super) dispatch_shape_sha256: [u8; 32],
}
impl fmt::Debug for ActiveSubmissionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveSubmissionV1")
            .field("id", &self.id)
            .field("stream", &self.stream)
            .field("ordered_predecessor", &self.ordered_predecessor)
            .field(
                "deferred_ordered_predecessor_retain",
                &self.deferred_ordered_predecessor_retain,
            )
            .field("kernel", &self.kernel)
            .field("allocations", &self.allocations)
            .field("writebacks", &self.writebacks)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(super) struct DataSpecV1 {
    pub(super) allocation: u64,
    pub(super) kind: RuntimeMemoryKindV1,
    pub(super) alignment: u64,
    pub(super) allocation_offset: u64,
    pub(super) bytes: Arc<[u8]>,
    pub(super) byte_range: Range<usize>,
    pub(super) content_sha256: Option<[u8; 32]>,
}

impl DataSpecV1 {
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes[self.byte_range.clone()]
    }

    pub(super) fn try_owned_bytes(&self) -> Result<Box<[u8]>, String> {
        let source = self.bytes();
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(source.len())
            .map_err(|_| "KFD native-data content allocation failed".to_owned())?;
        bytes.extend_from_slice(source);
        Ok(bytes.into_boxed_slice())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StagedPlacementV1 {
    pub(super) data_index: usize,
    pub(super) allocation_offset: u64,
}

#[derive(Debug)]
pub(super) struct StagedDataRosterV1 {
    pub(super) data: Vec<DataSpecV1>,
    pub(super) placements: HashMap<u64, StagedPlacementV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResidentDataDescriptorV1 {
    pub(super) allocation: u64,
    pub(super) kind: RuntimeMemoryKindV1,
    pub(super) alignment: u64,
    pub(super) allocation_offset: u64,
    pub(super) byte_len: u64,
    pub(super) host_content_sha256: Option<[u8; 32]>,
    pub(super) device_may_have_modified: bool,
}

pub(super) struct ResidentDataRosterV1 {
    pub(super) descriptors: Vec<ResidentDataDescriptorV1>,
    pub(super) data: Vec<Gfx942FixedDispatchDataV1>,
}

pub(super) struct RecycledDispatchV1 {
    pub(super) kernel: u64,
    pub(super) dispatch_shape_sha256: [u8; 32],
    pub(super) descriptors: Vec<ResidentDataDescriptorV1>,
}

pub(super) struct NativeComputeLaneRuntimeV1 {
    pub(super) owner_stream: Option<u64>,
    pub(super) active: Option<ActiveSubmissionV1>,
    pub(super) pipeline: RuntimeComputePipelineV1,
    pub(super) resident_data: Option<ResidentDataRosterV1>,
    pub(super) recycled_dispatch: Option<RecycledDispatchV1>,
}

impl NativeComputeLaneRuntimeV1 {
    pub(super) fn vacant() -> Self {
        Self {
            owner_stream: None,
            active: None,
            pipeline: RuntimeComputePipelineV1::vacant(),
            resident_data: None,
            recycled_dispatch: None,
        }
    }
}

pub(super) struct PreparedLaunchV1 {
    pub(super) stream: u64,
    pub(super) kernel: u64,
    pub(super) program: OwnedValidatedKernelEnvelope,
    pub(super) signature: [u8; 32],
    pub(super) kernarg: Box<[u8]>,
    pub(super) geometry: AqlDispatchGeometryV1,
    pub(super) dynamic_shared_bytes: u32,
    pub(super) buffer_bindings: Box<[Gfx942DispatchBufferBindingV1]>,
    pub(super) abi_rows: Vec<OwnedAbiRowV1>,
    pub(super) storage: PreparedLaunchStorageV1,
    pub(super) allocations: HashSet<u64>,
    pub(super) writebacks: Vec<WritebackV1>,
    pub(super) dispatch_shape_sha256: [u8; 32],
    pub(super) profile_launch: KfdProfileLaunchV1,
    pub(super) profile_semantic_contract: Option<KfdProfileSemanticContractV1>,
    pub(super) profile_bindings: Option<Result<Vec<KfdProfileBindingV1>, ()>>,
    pub(super) performance: KfdRuntimeLaunchPerformanceV1,
}

pub(super) enum PreparedLaunchStorageV1 {
    Materialized(Vec<DataSpecV1>),
    PersistentFullRange(PersistentFullRangePreparedV1),
    ThreeBindingPersistent(ThreeBindingPersistentPreparedV1),
}

pub(super) struct PersistentFullRangePreparedV1 {
    pub(super) allocation: u64,
    pub(super) access: RuntimeAccessV1,
    pub(super) source: PersistentFullRangeComputeSourceV1,
    pub(super) descriptors: Vec<ResidentDataDescriptorV1>,
}

pub(super) struct ThreeBindingPersistentPreparedV1 {
    pub(super) admissions: [PersistentFullRangeComputeAdmissionV1; 3],
    pub(super) descriptors: Vec<ResidentDataDescriptorV1>,
}
#[derive(Debug)]
pub(super) struct OwnedAbiRowV1 {
    pub(super) explicit_argument_index: usize,
    pub(super) offset: u64,
    pub(super) pointee_alignment: u64,
    pub(super) access: ArgumentAccess,
}

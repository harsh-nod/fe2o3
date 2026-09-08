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
    pub(super) prior_stream_submission: Option<u64>,
    pub(super) kernel: u64,
    pub(super) dependency_depth: usize,
    pub(super) allocations: HashSet<u64>,
    pub(super) writebacks: Vec<WritebackV1>,
    pub(super) resident_descriptors: Vec<ResidentDataDescriptorV1>,
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
    Materialized(Gfx942DispatchBatchV1<1>),
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

#[derive(Debug)]
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
    pub(super) launch: OwnedComputeLaunchV1,
    pub(super) retained_allocations: Box<[u64]>,
    pub(super) prior_stream_submission: Option<u64>,
    pub(super) dependencies: Vec<u64>,
    pub(super) dependency_cursor: usize,
    pub(super) dependency_depth: usize,
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
            .field("prior_stream_submission", &self.prior_stream_submission)
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
    pub(super) resident_data: Option<ResidentDataRosterV1>,
    pub(super) recycled_dispatch: Option<RecycledDispatchV1>,
}

impl NativeComputeLaneRuntimeV1 {
    pub(super) const fn vacant() -> Self {
        Self {
            owner_stream: None,
            active: None,
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

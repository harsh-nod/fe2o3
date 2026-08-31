//! Syscall-free device-local transfer and private-segment scratch admission.
//!
//! These values are model-only. They bind exact memory, queue, target, and
//! post-link metadata identities, but grant no native allocation, copy,
//! dispatch, completion, or hardware authority.

use crate::*;

pub const DEVICE_LOCAL_MODEL_SCHEMA_VERSION_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferDirectionV1 {
    Upload,
    Download,
}

/// Copy mechanism named by an external runtime contract.
///
/// The artifact identity is only a model binding. Authentication and machine
/// refinement remain obligations of the adapter that consumes this plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferMechanismV1 {
    CopyKernel {
        artifact: RuntimeArtifactIdV1,
        contract_identity: IdentityDigestV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceLocalTransferSliceV1 {
    mapping: MemoryMappingKeyV1,
    byte_offset: u64,
}

impl DeviceLocalTransferSliceV1 {
    pub const fn new(mapping: MemoryMappingKeyV1, byte_offset: u64) -> Self {
        Self {
            mapping,
            byte_offset,
        }
    }

    pub const fn mapping(self) -> MemoryMappingKeyV1 {
        self.mapping
    }

    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceLocalTransferRequestV1 {
    transfer_id: u64,
    queue: QueueKeyV1,
    direction: DeviceLocalTransferDirectionV1,
    mechanism: DeviceLocalTransferMechanismV1,
    source: DeviceLocalTransferSliceV1,
    destination: DeviceLocalTransferSliceV1,
    byte_len: u64,
    required_alignment: u64,
}

impl DeviceLocalTransferRequestV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        transfer_id: u64,
        queue: QueueKeyV1,
        direction: DeviceLocalTransferDirectionV1,
        mechanism: DeviceLocalTransferMechanismV1,
        source: DeviceLocalTransferSliceV1,
        destination: DeviceLocalTransferSliceV1,
        byte_len: u64,
        required_alignment: u64,
    ) -> Self {
        Self {
            transfer_id,
            queue,
            direction,
            mechanism,
            source,
            destination,
            byte_len,
            required_alignment,
        }
    }

    pub const fn transfer_id(self) -> u64 {
        self.transfer_id
    }

    pub const fn queue(self) -> QueueKeyV1 {
        self.queue
    }

    pub const fn direction(self) -> DeviceLocalTransferDirectionV1 {
        self.direction
    }

    pub const fn mechanism(self) -> DeviceLocalTransferMechanismV1 {
        self.mechanism
    }

    pub const fn source(self) -> DeviceLocalTransferSliceV1 {
        self.source
    }

    pub const fn destination(self) -> DeviceLocalTransferSliceV1 {
        self.destination
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub const fn required_alignment(self) -> u64 {
        self.required_alignment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedDeviceLocalTransferV1 {
    request: DeviceLocalTransferRequestV1,
}

impl AdmittedDeviceLocalTransferV1 {
    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn request(self) -> DeviceLocalTransferRequestV1 {
        self.request
    }

    pub const fn begin(self) -> DeviceLocalTransferStateV1 {
        DeviceLocalTransferStateV1 {
            plan: self,
            phase: DeviceLocalTransferPhaseV1::Planned,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferAdmissionErrorV1 {
    InvalidMemoryState,
    QueueStateInvalid,
    QueueNotActive,
    InvalidIdentity,
    InvalidRange,
    InvalidAlignment,
    AliasedEndpoints,
    BindingMismatch,
    UnsupportedMemoryKinds,
    InvalidAccess,
}

/// Admits one exact host-visible/device-local transfer against live model
/// memory and an active queue incarnation.
pub fn admit_device_local_transfer_v1(
    identity: &DeviceIdentityStateV1,
    queues: &QueueLifecycleStateV1,
    memory: &MemoryLifecycleStateV1,
    request: DeviceLocalTransferRequestV1,
) -> Result<AdmittedDeviceLocalTransferV1, DeviceLocalTransferAdmissionErrorV1> {
    memory
        .validate_global_invariants()
        .map_err(|_| DeviceLocalTransferAdmissionErrorV1::InvalidMemoryState)?;
    queues
        .validate_global_invariants(identity, memory)
        .map_err(|_| DeviceLocalTransferAdmissionErrorV1::QueueStateInvalid)?;
    require_active_queue(queues, request.queue)?;

    if request.transfer_id == 0
        || request.queue.id.0 == 0
        || request.queue.generation.0 == 0
        || request.byte_len == 0
        || mechanism_has_zero_identity(request.mechanism)
    {
        return Err(DeviceLocalTransferAdmissionErrorV1::InvalidIdentity);
    }
    if request.required_alignment == 0 || !request.required_alignment.is_power_of_two() {
        return Err(DeviceLocalTransferAdmissionErrorV1::InvalidAlignment);
    }
    if request.source.mapping == request.destination.mapping {
        return Err(DeviceLocalTransferAdmissionErrorV1::AliasedEndpoints);
    }

    let source = live_memory_binding(memory, request.source.mapping)?;
    let destination = live_memory_binding(memory, request.destination.mapping)?;
    if source.vm != request.queue.vm
        || destination.vm != request.queue.vm
        || source.device != request.queue.vm.device
        || destination.device != request.queue.vm.device
    {
        return Err(DeviceLocalTransferAdmissionErrorV1::BindingMismatch);
    }
    let (source_kind, source_coherence, destination_kind, destination_coherence) =
        match request.direction {
            DeviceLocalTransferDirectionV1::Upload => (
                MemoryKindV1::HostVisibleCoherent,
                MemoryCoherenceV1::HostCoherent,
                MemoryKindV1::DeviceLocal,
                MemoryCoherenceV1::ExplicitVisibility,
            ),
            DeviceLocalTransferDirectionV1::Download => (
                MemoryKindV1::DeviceLocal,
                MemoryCoherenceV1::ExplicitVisibility,
                MemoryKindV1::HostVisibleCoherent,
                MemoryCoherenceV1::HostCoherent,
            ),
        };
    if source.kind != source_kind
        || source.coherence != source_coherence
        || destination.kind != destination_kind
        || destination.coherence != destination_coherence
    {
        return Err(DeviceLocalTransferAdmissionErrorV1::UnsupportedMemoryKinds);
    }
    if !source.access.permits(MemoryAccessV1::Read)
        || !destination.access.permits(MemoryAccessV1::ReadWrite)
    {
        return Err(DeviceLocalTransferAdmissionErrorV1::InvalidAccess);
    }
    if source.alignment < request.required_alignment
        || destination.alignment < request.required_alignment
    {
        return Err(DeviceLocalTransferAdmissionErrorV1::InvalidAlignment);
    }
    validate_transfer_range(
        request.source,
        request.byte_len,
        request.required_alignment,
        source.byte_len,
    )?;
    validate_transfer_range(
        request.destination,
        request.byte_len,
        request.required_alignment,
        destination.byte_len,
    )?;

    Ok(AdmittedDeviceLocalTransferV1 { request })
}

fn mechanism_has_zero_identity(mechanism: DeviceLocalTransferMechanismV1) -> bool {
    match mechanism {
        DeviceLocalTransferMechanismV1::CopyKernel {
            artifact,
            contract_identity,
        } => {
            artifact.digest().as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
                || contract_identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
        }
    }
}

fn validate_transfer_range(
    slice: DeviceLocalTransferSliceV1,
    byte_len: u64,
    alignment: u64,
    allocation_bytes: u64,
) -> Result<(), DeviceLocalTransferAdmissionErrorV1> {
    if !slice.byte_offset.is_multiple_of(alignment) {
        return Err(DeviceLocalTransferAdmissionErrorV1::InvalidAlignment);
    }
    if slice
        .byte_offset
        .checked_add(byte_len)
        .is_none_or(|end| end > allocation_bytes)
    {
        return Err(DeviceLocalTransferAdmissionErrorV1::InvalidRange);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct LiveMemoryBindingV1 {
    vm: VmKeyV1,
    device: DeviceKeyV1,
    byte_len: u64,
    alignment: u64,
    kind: MemoryKindV1,
    coherence: MemoryCoherenceV1,
    access: MemoryAccessV1,
}

fn live_memory_binding(
    memory: &MemoryLifecycleStateV1,
    key: MemoryMappingKeyV1,
) -> Result<LiveMemoryBindingV1, DeviceLocalTransferAdmissionErrorV1> {
    let mapping = memory
        .mappings()
        .iter()
        .find(|record| record.key == key)
        .ok_or(DeviceLocalTransferAdmissionErrorV1::BindingMismatch)?;
    let allocation = memory
        .allocations()
        .iter()
        .find(|record| record.key == key.allocation)
        .ok_or(DeviceLocalTransferAdmissionErrorV1::BindingMismatch)?;
    if mapping.state != MemoryMappingStateV1::Mapped
        || allocation.state != MemoryAllocationStateV1::Live
        || mapping.target_devices.as_slice() != [key.allocation.vm.device]
    {
        return Err(DeviceLocalTransferAdmissionErrorV1::BindingMismatch);
    }
    Ok(LiveMemoryBindingV1 {
        vm: key.allocation.vm,
        device: mapping.target_devices[0],
        byte_len: allocation.spec.byte_len,
        alignment: allocation.spec.alignment,
        kind: allocation.spec.kind,
        coherence: allocation.spec.coherence,
        access: mapping.access,
    })
}

fn require_active_queue(
    queues: &QueueLifecycleStateV1,
    queue: QueueKeyV1,
) -> Result<(), DeviceLocalTransferAdmissionErrorV1> {
    let record = queues
        .queues()
        .iter()
        .find(|record| record.plan.queue == queue)
        .ok_or(DeviceLocalTransferAdmissionErrorV1::QueueNotActive)?;
    if record.phase != ComputeAqlQueuePhaseV1::Active {
        return Err(DeviceLocalTransferAdmissionErrorV1::QueueNotActive);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceLocalTransferPublicationV1 {
    transfer_id: u64,
    queue: QueueKeyV1,
    dispatch: DispatchIdV1,
    submission_sequence: u64,
}

impl DeviceLocalTransferPublicationV1 {
    pub const fn new(
        transfer_id: u64,
        queue: QueueKeyV1,
        dispatch: DispatchIdV1,
        submission_sequence: u64,
    ) -> Self {
        Self {
            transfer_id,
            queue,
            dispatch,
            submission_sequence,
        }
    }

    pub const fn transfer_id(self) -> u64 {
        self.transfer_id
    }

    pub const fn queue(self) -> QueueKeyV1 {
        self.queue
    }

    pub const fn dispatch(self) -> DispatchIdV1 {
        self.dispatch
    }

    pub const fn submission_sequence(self) -> u64 {
        self.submission_sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceLocalTransferCompletionV1 {
    publication: DeviceLocalTransferPublicationV1,
    completion: CompletionIdV1,
    acquire_sequence: u64,
}

impl DeviceLocalTransferCompletionV1 {
    pub const fn new(
        publication: DeviceLocalTransferPublicationV1,
        completion: CompletionIdV1,
        acquire_sequence: u64,
    ) -> Self {
        Self {
            publication,
            completion,
            acquire_sequence,
        }
    }

    pub const fn publication(self) -> DeviceLocalTransferPublicationV1 {
        self.publication
    }

    pub const fn completion(self) -> CompletionIdV1 {
        self.completion
    }

    pub const fn acquire_sequence(self) -> u64 {
        self.acquire_sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferPhaseV1 {
    Planned,
    Published(DeviceLocalTransferPublicationV1),
    Completed(DeviceLocalTransferCompletionV1),
    CancelledBeforePublication,
    Ambiguous(DeviceLocalTransferPublicationV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferTransitionV1 {
    Publish(DeviceLocalTransferPublicationV1),
    ObserveCompletion(DeviceLocalTransferCompletionV1),
    CancelBeforePublication,
    MarkAmbiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferTransitionErrorV1 {
    IllegalTransition,
    PublicationMismatch,
    InvalidOrdering,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceLocalTransferStateV1 {
    plan: AdmittedDeviceLocalTransferV1,
    phase: DeviceLocalTransferPhaseV1,
}

impl DeviceLocalTransferStateV1 {
    pub const fn plan(self) -> AdmittedDeviceLocalTransferV1 {
        self.plan
    }

    pub const fn phase(self) -> DeviceLocalTransferPhaseV1 {
        self.phase
    }

    pub fn next(
        self,
        transition: DeviceLocalTransferTransitionV1,
    ) -> Result<Self, DeviceLocalTransferTransitionErrorV1> {
        let phase = match (self.phase, transition) {
            (
                DeviceLocalTransferPhaseV1::Planned,
                DeviceLocalTransferTransitionV1::Publish(publication),
            ) => {
                let request = self.plan.request;
                if publication.transfer_id != request.transfer_id
                    || publication.queue != request.queue
                    || publication.dispatch.0 == 0
                    || publication.submission_sequence == 0
                {
                    return Err(DeviceLocalTransferTransitionErrorV1::PublicationMismatch);
                }
                DeviceLocalTransferPhaseV1::Published(publication)
            }
            (
                DeviceLocalTransferPhaseV1::Published(publication),
                DeviceLocalTransferTransitionV1::ObserveCompletion(completion),
            ) => {
                if completion.publication != publication || completion.completion.0 == 0 {
                    return Err(DeviceLocalTransferTransitionErrorV1::PublicationMismatch);
                }
                if completion.acquire_sequence <= publication.submission_sequence {
                    return Err(DeviceLocalTransferTransitionErrorV1::InvalidOrdering);
                }
                DeviceLocalTransferPhaseV1::Completed(completion)
            }
            (
                DeviceLocalTransferPhaseV1::Planned,
                DeviceLocalTransferTransitionV1::CancelBeforePublication,
            ) => DeviceLocalTransferPhaseV1::CancelledBeforePublication,
            (
                DeviceLocalTransferPhaseV1::Published(publication),
                DeviceLocalTransferTransitionV1::MarkAmbiguous,
            ) => DeviceLocalTransferPhaseV1::Ambiguous(publication),
            _ => return Err(DeviceLocalTransferTransitionErrorV1::IllegalTransition),
        };
        Ok(Self { phase, ..self })
    }

    /// Returns model-only visibility after an exact upload completion.
    pub const fn device_visibility(self) -> Option<AdmittedDeviceLocalTransferV1> {
        if matches!(self.phase, DeviceLocalTransferPhaseV1::Completed(_))
            && matches!(
                self.plan.request.direction,
                DeviceLocalTransferDirectionV1::Upload
            )
        {
            Some(self.plan)
        } else {
            None
        }
    }

    /// Returns model-only host visibility after an exact download completion.
    pub const fn host_visibility(self) -> Option<AdmittedDeviceLocalTransferV1> {
        if matches!(self.phase, DeviceLocalTransferPhaseV1::Completed(_))
            && matches!(
                self.plan.request.direction,
                DeviceLocalTransferDirectionV1::Download
            )
        {
            Some(self.plan)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateSegmentTargetContractV1 {
    contract_identity: IdentityDigestV1,
    target_identity: IdentityDigestV1,
    queue_target: ComputeAqlTargetProfileV1,
    wavefront_size: u32,
    maximum_resident_workgroups: u32,
    scratch_alignment: u64,
    max_private_bytes_per_workitem: u64,
    max_scratch_bytes_per_wave: u64,
    max_scratch_bytes_per_workgroup: u64,
    max_scratch_bytes_per_queue: u64,
}

impl PrivateSegmentTargetContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_identity: IdentityDigestV1,
        target_identity: IdentityDigestV1,
        queue_target: ComputeAqlTargetProfileV1,
        wavefront_size: u32,
        maximum_resident_workgroups: u32,
        scratch_alignment: u64,
        max_private_bytes_per_workitem: u64,
        max_scratch_bytes_per_wave: u64,
        max_scratch_bytes_per_workgroup: u64,
        max_scratch_bytes_per_queue: u64,
    ) -> Result<Self, PrivateSegmentAdmissionErrorV1> {
        if contract_identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
            || target_identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
        {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidIdentity);
        }
        if wavefront_size == 0 || !wavefront_size.is_power_of_two() {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidWavefrontSize);
        }
        if maximum_resident_workgroups == 0 {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidTargetBounds);
        }
        if scratch_alignment == 0 || !scratch_alignment.is_power_of_two() {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidAlignment);
        }
        if max_private_bytes_per_workitem == 0
            || max_scratch_bytes_per_wave == 0
            || max_scratch_bytes_per_workgroup == 0
            || max_scratch_bytes_per_queue == 0
            || max_scratch_bytes_per_wave > max_scratch_bytes_per_workgroup
            || max_scratch_bytes_per_workgroup > max_scratch_bytes_per_queue
        {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidTargetBounds);
        }
        Ok(Self {
            contract_identity,
            target_identity,
            queue_target,
            wavefront_size,
            maximum_resident_workgroups,
            scratch_alignment,
            max_private_bytes_per_workitem,
            max_scratch_bytes_per_wave,
            max_scratch_bytes_per_workgroup,
            max_scratch_bytes_per_queue,
        })
    }

    pub const fn contract_identity(self) -> IdentityDigestV1 {
        self.contract_identity
    }

    pub const fn target_identity(self) -> IdentityDigestV1 {
        self.target_identity
    }

    pub const fn queue_target(self) -> ComputeAqlTargetProfileV1 {
        self.queue_target
    }

    pub const fn wavefront_size(self) -> u32 {
        self.wavefront_size
    }

    pub const fn maximum_resident_workgroups(self) -> u32 {
        self.maximum_resident_workgroups
    }

    pub const fn scratch_alignment(self) -> u64 {
        self.scratch_alignment
    }

    pub const fn max_private_bytes_per_workitem(self) -> u64 {
        self.max_private_bytes_per_workitem
    }

    pub const fn max_scratch_bytes_per_wave(self) -> u64 {
        self.max_scratch_bytes_per_wave
    }

    pub const fn max_scratch_bytes_per_workgroup(self) -> u64 {
        self.max_scratch_bytes_per_workgroup
    }

    pub const fn max_scratch_bytes_per_queue(self) -> u64 {
        self.max_scratch_bytes_per_queue
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostLinkPrivateSegmentMetadataV1 {
    artifact: RuntimeArtifactIdV1,
    kernel_identity: IdentityDigestV1,
    metadata_identity: IdentityDigestV1,
    private_segment_fixed_bytes: u64,
}

impl PostLinkPrivateSegmentMetadataV1 {
    pub fn new(
        artifact: RuntimeArtifactIdV1,
        kernel_identity: IdentityDigestV1,
        metadata_identity: IdentityDigestV1,
        private_segment_fixed_bytes: u64,
    ) -> Result<Self, PrivateSegmentAdmissionErrorV1> {
        if artifact.digest().as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
            || kernel_identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
            || metadata_identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
        {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidIdentity);
        }
        Ok(Self {
            artifact,
            kernel_identity,
            metadata_identity,
            private_segment_fixed_bytes,
        })
    }

    pub const fn private_segment_fixed_bytes(self) -> u64 {
        self.private_segment_fixed_bytes
    }

    pub const fn artifact(self) -> RuntimeArtifactIdV1 {
        self.artifact
    }

    pub const fn kernel_identity(self) -> IdentityDigestV1 {
        self.kernel_identity
    }

    pub const fn metadata_identity(self) -> IdentityDigestV1 {
        self.metadata_identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateSegmentDispatchShapeV1 {
    workitems_per_workgroup: u32,
}

impl PrivateSegmentDispatchShapeV1 {
    pub const fn new(workitems_per_workgroup: u32) -> Self {
        Self {
            workitems_per_workgroup,
        }
    }

    pub const fn workitems_per_workgroup(self) -> u32 {
        self.workitems_per_workgroup
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateSegmentScratchPlanV1 {
    target: PrivateSegmentTargetContractV1,
    metadata: PostLinkPrivateSegmentMetadataV1,
    queue: QueueKeyV1,
    scratch_mapping: MemoryMappingKeyV1,
    shape: PrivateSegmentDispatchShapeV1,
    wave_count_per_workgroup: u64,
    scratch_bytes_per_wave: u64,
    scratch_bytes_per_workgroup: u64,
    scratch_bytes_per_queue: u64,
    packet_private_segment_bytes: u32,
}

impl PrivateSegmentScratchPlanV1 {
    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn scratch_mapping(self) -> MemoryMappingKeyV1 {
        self.scratch_mapping
    }

    pub const fn target(self) -> PrivateSegmentTargetContractV1 {
        self.target
    }

    pub const fn metadata(self) -> PostLinkPrivateSegmentMetadataV1 {
        self.metadata
    }

    pub const fn shape(self) -> PrivateSegmentDispatchShapeV1 {
        self.shape
    }

    pub const fn scratch_bytes_per_wave(self) -> u64 {
        self.scratch_bytes_per_wave
    }

    pub const fn scratch_bytes_per_workgroup(self) -> u64 {
        self.scratch_bytes_per_workgroup
    }

    pub const fn scratch_bytes_per_queue(self) -> u64 {
        self.scratch_bytes_per_queue
    }

    pub const fn wave_count_per_workgroup(self) -> u64 {
        self.wave_count_per_workgroup
    }

    pub const fn packet_private_segment_bytes(self) -> u32 {
        self.packet_private_segment_bytes
    }

    pub fn require_current_metadata(
        self,
        metadata: PostLinkPrivateSegmentMetadataV1,
    ) -> Result<Self, PrivateSegmentAdmissionErrorV1> {
        if self.metadata != metadata {
            return Err(PrivateSegmentAdmissionErrorV1::PostLinkMetadataMismatch);
        }
        Ok(self)
    }

    pub fn require_current_target(
        self,
        target: PrivateSegmentTargetContractV1,
    ) -> Result<Self, PrivateSegmentAdmissionErrorV1> {
        if self.target != target {
            return Err(PrivateSegmentAdmissionErrorV1::TargetContractMismatch);
        }
        Ok(self)
    }

    pub const fn queue(self) -> QueueKeyV1 {
        self.queue
    }

    pub fn require_current_shape(
        self,
        shape: PrivateSegmentDispatchShapeV1,
    ) -> Result<Self, PrivateSegmentAdmissionErrorV1> {
        if self.shape != shape {
            return Err(PrivateSegmentAdmissionErrorV1::DispatchShapeMismatch);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateSegmentAdmissionV1 {
    NotRequired {
        target: PrivateSegmentTargetContractV1,
        metadata: PostLinkPrivateSegmentMetadataV1,
        queue: QueueKeyV1,
    },
    Required(PrivateSegmentScratchPlanV1),
}

impl PrivateSegmentAdmissionV1 {
    pub const fn packet_private_segment_bytes(self) -> u32 {
        match self {
            Self::NotRequired { .. } => 0,
            Self::Required(plan) => plan.packet_private_segment_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrivateSegmentAdmissionErrorV1 {
    InvalidMemoryState,
    QueueStateInvalid,
    QueueNotActive,
    InvalidIdentity,
    InvalidWavefrontSize,
    InvalidAlignment,
    InvalidTargetBounds,
    InvalidDispatchShape,
    ArithmeticOverflow,
    PrivateSegmentPacketOverflow,
    PrivateSegmentPerWorkitemExceeded,
    ScratchPerWaveExceeded,
    ScratchPerWorkgroupExceeded,
    ScratchPerQueueExceeded,
    MissingScratchMapping,
    UnexpectedScratchMapping,
    ScratchBindingMismatch,
    ScratchCapacityInsufficient,
    PostLinkMetadataMismatch,
    TargetContractMismatch,
    DispatchShapeMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateSegmentAdmissionRequestV1 {
    target: PrivateSegmentTargetContractV1,
    metadata: PostLinkPrivateSegmentMetadataV1,
    queue: QueueKeyV1,
    shape: PrivateSegmentDispatchShapeV1,
    scratch_mapping: Option<MemoryMappingKeyV1>,
}

impl PrivateSegmentAdmissionRequestV1 {
    pub const fn new(
        target: PrivateSegmentTargetContractV1,
        metadata: PostLinkPrivateSegmentMetadataV1,
        queue: QueueKeyV1,
        shape: PrivateSegmentDispatchShapeV1,
        scratch_mapping: Option<MemoryMappingKeyV1>,
    ) -> Self {
        Self {
            target,
            metadata,
            queue,
            shape,
            scratch_mapping,
        }
    }
}

/// Admits post-link private-segment metadata against explicit target bounds and
/// one exact live queue-owned scratch mapping.
pub fn admit_private_segment_scratch_v1(
    identity: &DeviceIdentityStateV1,
    queues: &QueueLifecycleStateV1,
    memory: &MemoryLifecycleStateV1,
    request: PrivateSegmentAdmissionRequestV1,
) -> Result<PrivateSegmentAdmissionV1, PrivateSegmentAdmissionErrorV1> {
    let PrivateSegmentAdmissionRequestV1 {
        target,
        metadata,
        queue,
        shape,
        scratch_mapping,
    } = request;
    memory
        .validate_global_invariants()
        .map_err(|_| PrivateSegmentAdmissionErrorV1::InvalidMemoryState)?;
    queues
        .validate_global_invariants(identity, memory)
        .map_err(|_| PrivateSegmentAdmissionErrorV1::QueueStateInvalid)?;
    let record = queues
        .queues()
        .iter()
        .find(|record| record.plan.queue == queue)
        .ok_or(PrivateSegmentAdmissionErrorV1::QueueNotActive)?;
    if record.phase != ComputeAqlQueuePhaseV1::Active {
        return Err(PrivateSegmentAdmissionErrorV1::QueueNotActive);
    }
    if record.plan.target != target.queue_target {
        return Err(PrivateSegmentAdmissionErrorV1::TargetContractMismatch);
    }
    if shape.workitems_per_workgroup == 0 {
        return Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape);
    }
    if metadata.private_segment_fixed_bytes == 0 {
        if scratch_mapping.is_some() {
            return Err(PrivateSegmentAdmissionErrorV1::UnexpectedScratchMapping);
        }
        return Ok(PrivateSegmentAdmissionV1::NotRequired {
            target,
            metadata,
            queue,
        });
    }
    if metadata.private_segment_fixed_bytes > target.max_private_bytes_per_workitem {
        return Err(PrivateSegmentAdmissionErrorV1::PrivateSegmentPerWorkitemExceeded);
    }
    let packet_private_segment_bytes = u32::try_from(metadata.private_segment_fixed_bytes)
        .map_err(|_| PrivateSegmentAdmissionErrorV1::PrivateSegmentPacketOverflow)?;
    let raw_wave_bytes = metadata
        .private_segment_fixed_bytes
        .checked_mul(u64::from(target.wavefront_size))
        .ok_or(PrivateSegmentAdmissionErrorV1::ArithmeticOverflow)?;
    let scratch_bytes_per_wave = checked_align_up(raw_wave_bytes, target.scratch_alignment)?;
    if scratch_bytes_per_wave > target.max_scratch_bytes_per_wave {
        return Err(PrivateSegmentAdmissionErrorV1::ScratchPerWaveExceeded);
    }
    let wave_count_per_workgroup = u64::from(shape.workitems_per_workgroup)
        .checked_add(u64::from(target.wavefront_size) - 1)
        .ok_or(PrivateSegmentAdmissionErrorV1::ArithmeticOverflow)?
        / u64::from(target.wavefront_size);
    let scratch_bytes_per_workgroup = scratch_bytes_per_wave
        .checked_mul(wave_count_per_workgroup)
        .ok_or(PrivateSegmentAdmissionErrorV1::ArithmeticOverflow)?;
    if scratch_bytes_per_workgroup > target.max_scratch_bytes_per_workgroup {
        return Err(PrivateSegmentAdmissionErrorV1::ScratchPerWorkgroupExceeded);
    }
    let scratch_bytes_per_queue = scratch_bytes_per_workgroup
        .checked_mul(u64::from(target.maximum_resident_workgroups))
        .ok_or(PrivateSegmentAdmissionErrorV1::ArithmeticOverflow)?;
    if scratch_bytes_per_queue > target.max_scratch_bytes_per_queue {
        return Err(PrivateSegmentAdmissionErrorV1::ScratchPerQueueExceeded);
    }
    let scratch_mapping =
        scratch_mapping.ok_or(PrivateSegmentAdmissionErrorV1::MissingScratchMapping)?;
    let binding = live_memory_binding(memory, scratch_mapping)
        .map_err(|_| PrivateSegmentAdmissionErrorV1::ScratchBindingMismatch)?;
    let allocation = memory
        .allocations()
        .iter()
        .find(|record| record.key == scratch_mapping.allocation)
        .ok_or(PrivateSegmentAdmissionErrorV1::ScratchBindingMismatch)?;
    if binding.vm != queue.vm
        || binding.device != queue.vm.device
        || binding.kind != MemoryKindV1::ScratchContextSave
        || binding.coherence != MemoryCoherenceV1::ExplicitVisibility
        || !binding.access.permits(MemoryAccessV1::ReadWrite)
        || allocation.spec.alignment < target.scratch_alignment
        || !allocation
            .spec
            .alignment
            .is_multiple_of(target.scratch_alignment)
    {
        return Err(PrivateSegmentAdmissionErrorV1::ScratchBindingMismatch);
    }
    if binding.byte_len < scratch_bytes_per_queue {
        return Err(PrivateSegmentAdmissionErrorV1::ScratchCapacityInsufficient);
    }
    Ok(PrivateSegmentAdmissionV1::Required(
        PrivateSegmentScratchPlanV1 {
            target,
            metadata,
            queue,
            scratch_mapping,
            shape,
            wave_count_per_workgroup,
            scratch_bytes_per_wave,
            scratch_bytes_per_workgroup,
            scratch_bytes_per_queue,
            packet_private_segment_bytes,
        },
    ))
}

fn checked_align_up(value: u64, alignment: u64) -> Result<u64, PrivateSegmentAdmissionErrorV1> {
    value
        .checked_add(alignment - 1)
        .map(|end| end & !(alignment - 1))
        .ok_or(PrivateSegmentAdmissionErrorV1::ArithmeticOverflow)
}

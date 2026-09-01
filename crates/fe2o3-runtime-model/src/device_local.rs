//! Syscall-free device-local transfer and private-segment scratch admission.
//!
//! These values are model-only. They bind exact memory, queue, target, and
//! post-link metadata identities, but grant no native allocation, copy,
//! dispatch, completion, or hardware authority.

use alloc::{boxed::Box, vec::Vec};

use crate::*;

pub const DEVICE_LOCAL_MODEL_SCHEMA_VERSION_V1: u16 = 1;
pub const GFX942_WAVEFRONT_SIZE_V1: u32 = 64;
pub const GFX942_MAX_FLAT_WORKGROUP_SIZE_V1: u32 = 1_024;

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

pub const MAX_DEVICE_LOCAL_TRANSFER_RECORDS_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceLocalTransferRetentionV1 {
    source: MemoryPublicationKeyV1,
    destination: MemoryPublicationKeyV1,
}

impl DeviceLocalTransferRetentionV1 {
    pub const fn new(source: MemoryPublicationKeyV1, destination: MemoryPublicationKeyV1) -> Self {
        Self {
            source,
            destination,
        }
    }

    pub const fn source(self) -> MemoryPublicationKeyV1 {
        self.source
    }

    pub const fn destination(self) -> MemoryPublicationKeyV1 {
        self.destination
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceLocalTransferBindingV1 {
    registry_incarnation: IdentityDigestV1,
    transfer_id: u64,
    queue: QueueKeyV1,
    dispatch: DispatchKeyV1,
    completion: CompletionKeyV1,
    retention: DeviceLocalTransferRetentionV1,
}

impl DeviceLocalTransferBindingV1 {
    pub const fn registry_incarnation(self) -> IdentityDigestV1 {
        self.registry_incarnation
    }

    pub const fn transfer_id(self) -> u64 {
        self.transfer_id
    }

    pub const fn queue(self) -> QueueKeyV1 {
        self.queue
    }

    pub const fn dispatch(self) -> DispatchKeyV1 {
        self.dispatch
    }

    pub const fn completion(self) -> CompletionKeyV1 {
        self.completion
    }

    pub const fn retention(self) -> DeviceLocalTransferRetentionV1 {
        self.retention
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferPhaseV1 {
    Reserved,
    Submitted { submission_sequence: u64 },
    VisibilityObserved { acquire_sequence: u64 },
    Indeterminate,
    Released,
}

impl DeviceLocalTransferPhaseV1 {
    const fn retains_memory(self) -> bool {
        !matches!(self, Self::Released)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeviceLocalTransferRecordV1 {
    binding: DeviceLocalTransferBindingV1,
    request: DeviceLocalTransferRequestV1,
    phase: DeviceLocalTransferPhaseV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferErrorV1 {
    InvalidRegistryIncarnation,
    CapacityExceeded,
    Admission(DeviceLocalTransferAdmissionErrorV1),
    InvalidDispatchIdentity,
    InvalidRetention,
    DuplicateIdentity,
    ResourceConflict,
    TokenMismatch,
    IllegalTransition,
    InvalidOrdering,
    Memory(MemoryTransitionErrorV1),
}

#[must_use]
pub struct DeviceLocalTransferRegistryCreateFailureV1 {
    error: DeviceLocalTransferErrorV1,
    memory: Box<MemoryLifecycleStateV1>,
    queues: Box<QueueLifecycleStateV1>,
}

impl DeviceLocalTransferRegistryCreateFailureV1 {
    pub const fn error(&self) -> DeviceLocalTransferErrorV1 {
        self.error
    }

    pub fn into_states(self) -> (MemoryLifecycleStateV1, QueueLifecycleStateV1) {
        (*self.memory, *self.queues)
    }
}

impl core::fmt::Debug for DeviceLocalTransferRegistryCreateFailureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DeviceLocalTransferRegistryCreateFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

pub struct DeviceLocalTransferRegistryV1 {
    memory: MemoryLifecycleStateV1,
    queues: QueueLifecycleStateV1,
    registry_incarnation: IdentityDigestV1,
    records: Vec<DeviceLocalTransferRecordV1>,
}

impl core::fmt::Debug for DeviceLocalTransferRegistryV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DeviceLocalTransferRegistryV1")
            .field("registry_incarnation", &self.registry_incarnation)
            .field("records", &self.records)
            .finish_non_exhaustive()
    }
}

impl DeviceLocalTransferRegistryV1 {
    pub fn new_model_only(
        identity: &DeviceIdentityStateV1,
        memory: MemoryLifecycleStateV1,
        queues: QueueLifecycleStateV1,
        registry_incarnation: IdentityDigestV1,
    ) -> Result<Self, DeviceLocalTransferRegistryCreateFailureV1> {
        let error = if registry_incarnation.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1] {
            Some(DeviceLocalTransferErrorV1::InvalidRegistryIncarnation)
        } else if let Err(error) = memory.validate_global_invariants() {
            Some(DeviceLocalTransferErrorV1::Memory(
                MemoryTransitionErrorV1::SourceInvariant(error),
            ))
        } else if queues
            .validate_global_invariants(identity, &memory)
            .is_err()
        {
            Some(DeviceLocalTransferErrorV1::Admission(
                DeviceLocalTransferAdmissionErrorV1::QueueStateInvalid,
            ))
        } else {
            None
        };
        if let Some(error) = error {
            return Err(DeviceLocalTransferRegistryCreateFailureV1 {
                error,
                memory: Box::new(memory),
                queues: Box::new(queues),
            });
        }
        Ok(Self {
            memory,
            queues,
            registry_incarnation,
            records: Vec::new(),
        })
    }

    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub fn memory_state(&self) -> &MemoryLifecycleStateV1 {
        &self.memory
    }

    pub fn queue_state(&self) -> &QueueLifecycleStateV1 {
        &self.queues
    }

    pub fn retained_transfer_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.phase.retains_memory())
            .count()
    }

    pub fn into_states(self) -> Result<(MemoryLifecycleStateV1, QueueLifecycleStateV1), Box<Self>> {
        if self.retained_transfer_count() == 0 {
            Ok((self.memory, self.queues))
        } else {
            Err(Box::new(self))
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_model_only(
        &mut self,
        identity: &DeviceIdentityStateV1,
        request: DeviceLocalTransferRequestV1,
        dispatch: DispatchKeyV1,
        completion: CompletionKeyV1,
        retention: DeviceLocalTransferRetentionV1,
    ) -> Result<DeviceLocalTransferReservedTokenV1, DeviceLocalTransferErrorV1> {
        if self.records.len() >= MAX_DEVICE_LOCAL_TRANSFER_RECORDS_V1 {
            return Err(DeviceLocalTransferErrorV1::CapacityExceeded);
        }
        validate_device_local_transfer_v1(identity, &self.queues, &self.memory, request)
            .map_err(DeviceLocalTransferErrorV1::Admission)?;
        if dispatch.queue != request.queue
            || dispatch.id.0 == 0
            || completion.dispatch != dispatch
            || completion.id.0 == 0
        {
            return Err(DeviceLocalTransferErrorV1::InvalidDispatchIdentity);
        }
        if retention.source.mapping != request.source.mapping
            || retention.destination.mapping != request.destination.mapping
            || retention.source.id.0 == 0
            || retention.destination.id.0 == 0
            || retention.source == retention.destination
        {
            return Err(DeviceLocalTransferErrorV1::InvalidRetention);
        }
        if self.records.iter().any(|record| {
            record.binding.transfer_id == request.transfer_id
                || record.binding.dispatch == dispatch
                || record.binding.completion == completion
                || record.binding.retention.source == retention.source
                || record.binding.retention.destination == retention.destination
        }) {
            return Err(DeviceLocalTransferErrorV1::DuplicateIdentity);
        }
        if self.records.iter().any(|record| {
            record.phase.retains_memory() && transfer_requests_conflict(record.request, request)
        }) {
            return Err(DeviceLocalTransferErrorV1::ResourceConflict);
        }

        let source_published = self
            .memory
            .next(MemoryTransitionV1::PublishMapping {
                key: retention.source,
            })
            .map_err(DeviceLocalTransferErrorV1::Memory)?;
        let retained = source_published
            .next(MemoryTransitionV1::PublishMapping {
                key: retention.destination,
            })
            .map_err(DeviceLocalTransferErrorV1::Memory)?;
        let binding = DeviceLocalTransferBindingV1 {
            registry_incarnation: self.registry_incarnation,
            transfer_id: request.transfer_id,
            queue: request.queue,
            dispatch,
            completion,
            retention,
        };
        self.memory = retained;
        self.records.push(DeviceLocalTransferRecordV1 {
            binding,
            request,
            phase: DeviceLocalTransferPhaseV1::Reserved,
        });
        Ok(DeviceLocalTransferReservedTokenV1 { binding, request })
    }

    fn validate_context(
        &self,
        identity: &DeviceIdentityStateV1,
        queue: QueueKeyV1,
    ) -> Result<(), DeviceLocalTransferErrorV1> {
        self.queues
            .validate_global_invariants(identity, &self.memory)
            .map_err(|_| {
                DeviceLocalTransferErrorV1::Admission(
                    DeviceLocalTransferAdmissionErrorV1::QueueStateInvalid,
                )
            })?;
        require_active_queue(&self.queues, queue).map_err(DeviceLocalTransferErrorV1::Admission)
    }

    fn record_index(
        &self,
        binding: DeviceLocalTransferBindingV1,
        request: DeviceLocalTransferRequestV1,
        phase: DeviceLocalTransferPhaseV1,
    ) -> Result<usize, DeviceLocalTransferErrorV1> {
        if binding.registry_incarnation != self.registry_incarnation {
            return Err(DeviceLocalTransferErrorV1::TokenMismatch);
        }
        self.records
            .iter()
            .position(|record| {
                record.binding == binding && record.request == request && record.phase == phase
            })
            .ok_or(DeviceLocalTransferErrorV1::TokenMismatch)
    }

    fn release_retention(
        &self,
        retention: DeviceLocalTransferRetentionV1,
    ) -> Result<MemoryLifecycleStateV1, DeviceLocalTransferErrorV1> {
        let source_released = self
            .memory
            .next(MemoryTransitionV1::ReleasePublication {
                key: retention.source,
            })
            .map_err(DeviceLocalTransferErrorV1::Memory)?;
        source_released
            .next(MemoryTransitionV1::ReleasePublication {
                key: retention.destination,
            })
            .map_err(DeviceLocalTransferErrorV1::Memory)
    }

    fn publish(
        &mut self,
        identity: &DeviceIdentityStateV1,
        binding: DeviceLocalTransferBindingV1,
        request: DeviceLocalTransferRequestV1,
        submission_sequence: u64,
    ) -> Result<(), DeviceLocalTransferErrorV1> {
        let index = self.record_index(binding, request, DeviceLocalTransferPhaseV1::Reserved)?;
        self.validate_context(identity, binding.queue)?;
        if submission_sequence == 0 {
            return Err(DeviceLocalTransferErrorV1::InvalidOrdering);
        }
        self.records[index].phase = DeviceLocalTransferPhaseV1::Submitted {
            submission_sequence,
        };
        Ok(())
    }

    fn cancel_reserved(
        &mut self,
        binding: DeviceLocalTransferBindingV1,
        request: DeviceLocalTransferRequestV1,
    ) -> Result<(), DeviceLocalTransferErrorV1> {
        let index = self.record_index(binding, request, DeviceLocalTransferPhaseV1::Reserved)?;
        let memory = self.release_retention(binding.retention)?;
        self.memory = memory;
        self.records[index].phase = DeviceLocalTransferPhaseV1::Released;
        Ok(())
    }

    fn poll(
        &mut self,
        identity: &DeviceIdentityStateV1,
        binding: DeviceLocalTransferBindingV1,
        request: DeviceLocalTransferRequestV1,
        submission_sequence: u64,
        observation: DeviceLocalTransferCompletionObservationV1,
    ) -> Result<DeviceLocalTransferPollTransitionV1, DeviceLocalTransferErrorV1> {
        let phase = DeviceLocalTransferPhaseV1::Submitted {
            submission_sequence,
        };
        let index = self.record_index(binding, request, phase)?;
        match observation {
            DeviceLocalTransferCompletionObservationV1::Pending => {
                self.validate_context(identity, binding.queue)?;
                Ok(DeviceLocalTransferPollTransitionV1::Pending)
            }
            DeviceLocalTransferCompletionObservationV1::Completed {
                completion,
                acquire_sequence,
            } => {
                self.validate_context(identity, binding.queue)?;
                if completion != binding.completion {
                    return Err(DeviceLocalTransferErrorV1::TokenMismatch);
                }
                if acquire_sequence <= submission_sequence {
                    return Err(DeviceLocalTransferErrorV1::InvalidOrdering);
                }
                self.records[index].phase =
                    DeviceLocalTransferPhaseV1::VisibilityObserved { acquire_sequence };
                Ok(DeviceLocalTransferPollTransitionV1::Completed {
                    acquire_sequence,
                    direction: self.records[index].request.direction,
                })
            }
            DeviceLocalTransferCompletionObservationV1::Indeterminate => {
                self.records[index].phase = DeviceLocalTransferPhaseV1::Indeterminate;
                Ok(DeviceLocalTransferPollTransitionV1::Indeterminate)
            }
        }
    }

    fn release_visibility(
        &mut self,
        identity: &DeviceIdentityStateV1,
        binding: DeviceLocalTransferBindingV1,
        request: DeviceLocalTransferRequestV1,
        acquire_sequence: u64,
    ) -> Result<(), DeviceLocalTransferErrorV1> {
        let index = self.record_index(
            binding,
            request,
            DeviceLocalTransferPhaseV1::VisibilityObserved { acquire_sequence },
        )?;
        self.validate_context(identity, binding.queue)?;
        let memory = self.release_retention(binding.retention)?;
        self.memory = memory;
        self.records[index].phase = DeviceLocalTransferPhaseV1::Released;
        Ok(())
    }
}

/// Admits one exact host-visible/device-local transfer against live model
/// memory and an active queue incarnation.
fn validate_device_local_transfer_v1(
    identity: &DeviceIdentityStateV1,
    queues: &QueueLifecycleStateV1,
    memory: &MemoryLifecycleStateV1,
    request: DeviceLocalTransferRequestV1,
) -> Result<(), DeviceLocalTransferAdmissionErrorV1> {
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

    Ok(())
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

fn transfer_requests_conflict(
    left: DeviceLocalTransferRequestV1,
    right: DeviceLocalTransferRequestV1,
) -> bool {
    let left_slices = [(left.source, false), (left.destination, true)];
    let right_slices = [(right.source, false), (right.destination, true)];
    left_slices.iter().any(|(left_slice, left_write)| {
        right_slices.iter().any(|(right_slice, right_write)| {
            (*left_write || *right_write)
                && transfer_slices_overlap(*left_slice, left.byte_len, *right_slice, right.byte_len)
        })
    })
}

fn transfer_slices_overlap(
    left: DeviceLocalTransferSliceV1,
    left_len: u64,
    right: DeviceLocalTransferSliceV1,
    right_len: u64,
) -> bool {
    if left.mapping.allocation != right.mapping.allocation {
        return false;
    }
    let Some(left_end) = left.byte_offset.checked_add(left_len) else {
        return true;
    };
    let Some(right_end) = right.byte_offset.checked_add(right_len) else {
        return true;
    };
    left.byte_offset < right_end && right.byte_offset < left_end
}

#[must_use]
pub struct DeviceLocalTransferTokenFailureV1<T> {
    error: DeviceLocalTransferErrorV1,
    retained: Box<T>,
}

impl<T> DeviceLocalTransferTokenFailureV1<T> {
    pub const fn error(&self) -> DeviceLocalTransferErrorV1 {
        self.error
    }

    pub fn into_retained(self) -> T {
        *self.retained
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for DeviceLocalTransferTokenFailureV1<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DeviceLocalTransferTokenFailureV1")
            .field("error", &self.error)
            .field("retained", &self.retained)
            .finish()
    }
}

#[derive(Debug)]
#[must_use = "a transfer reservation must be published or cancelled"]
pub struct DeviceLocalTransferReservedTokenV1 {
    binding: DeviceLocalTransferBindingV1,
    request: DeviceLocalTransferRequestV1,
}

impl DeviceLocalTransferReservedTokenV1 {
    pub const fn binding(&self) -> DeviceLocalTransferBindingV1 {
        self.binding
    }

    pub const fn request(&self) -> DeviceLocalTransferRequestV1 {
        self.request
    }

    pub fn publish_model_only(
        self,
        registry: &mut DeviceLocalTransferRegistryV1,
        identity: &DeviceIdentityStateV1,
        submission_sequence: u64,
    ) -> Result<DeviceLocalTransferSubmittedTokenV1, DeviceLocalTransferTokenFailureV1<Self>> {
        match registry.publish(identity, self.binding, self.request, submission_sequence) {
            Ok(()) => Ok(DeviceLocalTransferSubmittedTokenV1 {
                binding: self.binding,
                request: self.request,
                submission_sequence,
            }),
            Err(error) => Err(DeviceLocalTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }

    pub fn cancel_before_publication_model_only(
        self,
        registry: &mut DeviceLocalTransferRegistryV1,
    ) -> Result<DeviceLocalTransferReleasedReceiptV1, DeviceLocalTransferTokenFailureV1<Self>> {
        match registry.cancel_reserved(self.binding, self.request) {
            Ok(()) => Ok(DeviceLocalTransferReleasedReceiptV1 {
                binding: self.binding,
                request: self.request,
                acquire_sequence: None,
            }),
            Err(error) => Err(DeviceLocalTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferCompletionObservationV1 {
    Pending,
    Completed {
        completion: CompletionKeyV1,
        acquire_sequence: u64,
    },
    Indeterminate,
}

#[derive(Debug)]
#[must_use = "submitted transfer custody must be polled to a terminal observation"]
pub struct DeviceLocalTransferSubmittedTokenV1 {
    binding: DeviceLocalTransferBindingV1,
    request: DeviceLocalTransferRequestV1,
    submission_sequence: u64,
}

impl DeviceLocalTransferSubmittedTokenV1 {
    pub const fn binding(&self) -> DeviceLocalTransferBindingV1 {
        self.binding
    }

    pub const fn request(&self) -> DeviceLocalTransferRequestV1 {
        self.request
    }

    pub fn poll_model_only(
        self,
        registry: &mut DeviceLocalTransferRegistryV1,
        identity: &DeviceIdentityStateV1,
        observation: DeviceLocalTransferCompletionObservationV1,
    ) -> Result<DeviceLocalTransferPollV1, DeviceLocalTransferTokenFailureV1<Self>> {
        match registry.poll(
            identity,
            self.binding,
            self.request,
            self.submission_sequence,
            observation,
        ) {
            Ok(DeviceLocalTransferPollTransitionV1::Pending) => {
                Ok(DeviceLocalTransferPollV1::Pending(self))
            }
            Ok(DeviceLocalTransferPollTransitionV1::Completed {
                acquire_sequence,
                direction,
            }) => Ok(DeviceLocalTransferPollV1::Completed(
                DeviceLocalTransferVisibilityTokenV1 {
                    binding: self.binding,
                    request: self.request,
                    direction,
                    acquire_sequence,
                },
            )),
            Ok(DeviceLocalTransferPollTransitionV1::Indeterminate) => Ok(
                DeviceLocalTransferPollV1::Indeterminate(DeviceLocalTransferQuarantineV1 {
                    binding: self.binding,
                    request: self.request,
                }),
            ),
            Err(error) => Err(DeviceLocalTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }
}

enum DeviceLocalTransferPollTransitionV1 {
    Pending,
    Completed {
        acquire_sequence: u64,
        direction: DeviceLocalTransferDirectionV1,
    },
    Indeterminate,
}

#[derive(Debug)]
pub enum DeviceLocalTransferPollV1 {
    Pending(DeviceLocalTransferSubmittedTokenV1),
    Completed(DeviceLocalTransferVisibilityTokenV1),
    Indeterminate(DeviceLocalTransferQuarantineV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLocalTransferVisibilityV1 {
    Device,
    Host,
}

#[derive(Debug)]
#[must_use = "visibility custody retains both endpoint mappings until consumed"]
pub struct DeviceLocalTransferVisibilityTokenV1 {
    binding: DeviceLocalTransferBindingV1,
    request: DeviceLocalTransferRequestV1,
    direction: DeviceLocalTransferDirectionV1,
    acquire_sequence: u64,
}

impl DeviceLocalTransferVisibilityTokenV1 {
    pub const fn binding(&self) -> DeviceLocalTransferBindingV1 {
        self.binding
    }

    pub const fn request(&self) -> DeviceLocalTransferRequestV1 {
        self.request
    }

    pub const fn acquire_sequence(&self) -> u64 {
        self.acquire_sequence
    }

    pub const fn visibility(&self) -> DeviceLocalTransferVisibilityV1 {
        match self.direction {
            DeviceLocalTransferDirectionV1::Upload => DeviceLocalTransferVisibilityV1::Device,
            DeviceLocalTransferDirectionV1::Download => DeviceLocalTransferVisibilityV1::Host,
        }
    }

    pub fn release_after_visibility_consumed_model_only(
        self,
        registry: &mut DeviceLocalTransferRegistryV1,
        identity: &DeviceIdentityStateV1,
    ) -> Result<DeviceLocalTransferReleasedReceiptV1, DeviceLocalTransferTokenFailureV1<Self>> {
        match registry.release_visibility(
            identity,
            self.binding,
            self.request,
            self.acquire_sequence,
        ) {
            Ok(()) => Ok(DeviceLocalTransferReleasedReceiptV1 {
                binding: self.binding,
                request: self.request,
                acquire_sequence: Some(self.acquire_sequence),
            }),
            Err(error) => Err(DeviceLocalTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }
}

#[derive(Debug)]
#[must_use = "indeterminate transfer custody has no release transition"]
pub struct DeviceLocalTransferQuarantineV1 {
    binding: DeviceLocalTransferBindingV1,
    request: DeviceLocalTransferRequestV1,
}

impl DeviceLocalTransferQuarantineV1 {
    pub const fn binding(&self) -> DeviceLocalTransferBindingV1 {
        self.binding
    }

    pub const fn request(&self) -> DeviceLocalTransferRequestV1 {
        self.request
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceLocalTransferReleasedReceiptV1 {
    binding: DeviceLocalTransferBindingV1,
    request: DeviceLocalTransferRequestV1,
    acquire_sequence: Option<u64>,
}

impl DeviceLocalTransferReleasedReceiptV1 {
    pub const fn binding(self) -> DeviceLocalTransferBindingV1 {
        self.binding
    }

    pub const fn request(self) -> DeviceLocalTransferRequestV1 {
        self.request
    }

    pub const fn acquire_sequence(self) -> Option<u64> {
        self.acquire_sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateSegmentTargetContractV1 {
    contract_identity: IdentityDigestV1,
    device_profile: DeviceAdmissionProfileIdV1,
    queue_plan: QueuePlanIdV1,
    queue_target: ComputeAqlTargetProfileV1,
    maximum_resident_workgroups: u32,
    scratch_alignment: u64,
    max_private_bytes_per_workitem: u64,
    max_scratch_bytes_per_wave: u64,
    max_scratch_bytes_per_workgroup: u64,
    max_scratch_bytes_per_queue: u64,
}

impl PrivateSegmentTargetContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn gfx942_model_only(
        contract_identity: IdentityDigestV1,
        device_profile: DeviceAdmissionProfileIdV1,
        queue_plan: QueuePlanIdV1,
        maximum_resident_workgroups: u32,
        scratch_alignment: u64,
        max_private_bytes_per_workitem: u64,
        max_scratch_bytes_per_wave: u64,
        max_scratch_bytes_per_workgroup: u64,
        max_scratch_bytes_per_queue: u64,
    ) -> Result<Self, PrivateSegmentAdmissionErrorV1> {
        if contract_identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
            || device_profile.digest().as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
            || queue_plan.digest().as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
        {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidIdentity);
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
            device_profile,
            queue_plan,
            queue_target: ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
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

    pub const fn device_profile(self) -> DeviceAdmissionProfileIdV1 {
        self.device_profile
    }

    pub const fn queue_plan(self) -> QueuePlanIdV1 {
        self.queue_plan
    }

    pub const fn queue_target(self) -> ComputeAqlTargetProfileV1 {
        self.queue_target
    }

    pub const fn wavefront_size(self) -> u32 {
        GFX942_WAVEFRONT_SIZE_V1
    }

    pub const fn max_flat_workgroup_size(self) -> u32 {
        GFX942_MAX_FLAT_WORKGROUP_SIZE_V1
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
    wavefront_size: u32,
    required_workgroup_size: Option<[u32; 3]>,
    max_flat_workgroup_size: u32,
    max_workgroups: [u32; 3],
    requires_uniform_workgroups: bool,
}

impl PostLinkPrivateSegmentMetadataV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        artifact: RuntimeArtifactIdV1,
        kernel_identity: IdentityDigestV1,
        metadata_identity: IdentityDigestV1,
        private_segment_fixed_bytes: u64,
        wavefront_size: u32,
        required_workgroup_size: Option<[u32; 3]>,
        max_flat_workgroup_size: u32,
        max_workgroups: [u32; 3],
        requires_uniform_workgroups: bool,
    ) -> Result<Self, PrivateSegmentAdmissionErrorV1> {
        if artifact.digest().as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
            || kernel_identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
            || metadata_identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
        {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidIdentity);
        }
        if wavefront_size == 0
            || max_flat_workgroup_size == 0
            || max_workgroups.contains(&0)
            || required_workgroup_size.is_some_and(|dimensions| {
                dimensions.contains(&0) || checked_dimension_product(dimensions).is_none()
            })
        {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape);
        }
        Ok(Self {
            artifact,
            kernel_identity,
            metadata_identity,
            private_segment_fixed_bytes,
            wavefront_size,
            required_workgroup_size,
            max_flat_workgroup_size,
            max_workgroups,
            requires_uniform_workgroups,
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

    pub const fn wavefront_size(self) -> u32 {
        self.wavefront_size
    }

    pub const fn required_workgroup_size(self) -> Option<[u32; 3]> {
        self.required_workgroup_size
    }

    pub const fn max_flat_workgroup_size(self) -> u32 {
        self.max_flat_workgroup_size
    }

    pub const fn max_workgroups(self) -> [u32; 3] {
        self.max_workgroups
    }

    pub const fn requires_uniform_workgroups(self) -> bool {
        self.requires_uniform_workgroups
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrivateSegmentDispatchShapeV1 {
    grid: [u32; 3],
    workgroup: [u32; 3],
    workitems_per_workgroup: u32,
}

impl PrivateSegmentDispatchShapeV1 {
    pub fn new(
        grid: [u32; 3],
        workgroup: [u32; 3],
    ) -> Result<Self, PrivateSegmentAdmissionErrorV1> {
        if grid.contains(&0)
            || workgroup.contains(&0)
            || grid
                .iter()
                .zip(workgroup.iter())
                .any(|(grid, workgroup)| grid < workgroup)
        {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape);
        }
        let workitems_per_workgroup = checked_dimension_product(workgroup)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape)?;
        Ok(Self {
            grid,
            workgroup,
            workitems_per_workgroup,
        })
    }

    pub const fn grid(self) -> [u32; 3] {
        self.grid
    }

    pub const fn workgroup(self) -> [u32; 3] {
        self.workgroup
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
}

impl PrivateSegmentAdmissionRequestV1 {
    pub const fn new(
        target: PrivateSegmentTargetContractV1,
        metadata: PostLinkPrivateSegmentMetadataV1,
        queue: QueueKeyV1,
        shape: PrivateSegmentDispatchShapeV1,
    ) -> Self {
        Self {
            target,
            metadata,
            queue,
            shape,
        }
    }
}

/// Admits post-link private-segment metadata against explicit model policy and
/// the exact scratch mapping retained by the active queue plan.
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
    if record.plan.target != target.queue_target
        || record.plan.plan_id != target.queue_plan
        || record.plan.current_device.correlation().profile_id() != target.device_profile
    {
        return Err(PrivateSegmentAdmissionErrorV1::TargetContractMismatch);
    }
    if shape.workitems_per_workgroup == 0
        || shape.workitems_per_workgroup > target.max_flat_workgroup_size()
        || metadata.wavefront_size != target.wavefront_size()
        || metadata.max_flat_workgroup_size == 0
        || metadata.max_flat_workgroup_size > target.max_flat_workgroup_size()
        || shape.workitems_per_workgroup > metadata.max_flat_workgroup_size
        || metadata
            .required_workgroup_size
            .is_some_and(|required| required != shape.workgroup)
    {
        return Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape);
    }
    for axis in 0..3 {
        let workgroups = shape.grid[axis].div_ceil(shape.workgroup[axis]);
        if workgroups > metadata.max_workgroups[axis]
            || (metadata.requires_uniform_workgroups
                && !shape.grid[axis].is_multiple_of(shape.workgroup[axis]))
        {
            return Err(PrivateSegmentAdmissionErrorV1::InvalidDispatchShape);
        }
    }
    if metadata.private_segment_fixed_bytes == 0 {
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
        .checked_mul(u64::from(target.wavefront_size()))
        .ok_or(PrivateSegmentAdmissionErrorV1::ArithmeticOverflow)?;
    let scratch_bytes_per_wave = checked_align_up(raw_wave_bytes, target.scratch_alignment)?;
    if scratch_bytes_per_wave > target.max_scratch_bytes_per_wave {
        return Err(PrivateSegmentAdmissionErrorV1::ScratchPerWaveExceeded);
    }
    let wave_count_per_workgroup = u64::from(shape.workitems_per_workgroup)
        .checked_add(u64::from(target.wavefront_size()) - 1)
        .ok_or(PrivateSegmentAdmissionErrorV1::ArithmeticOverflow)?
        / u64::from(target.wavefront_size());
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
    let scratch_mapping = record
        .plan
        .resources
        .private_scratch
        .ok_or(PrivateSegmentAdmissionErrorV1::MissingScratchMapping)?
        .mapping;
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

fn checked_dimension_product(dimensions: [u32; 3]) -> Option<u64> {
    u64::from(dimensions[0])
        .checked_mul(u64::from(dimensions[1]))?
        .checked_mul(u64::from(dimensions[2]))
}

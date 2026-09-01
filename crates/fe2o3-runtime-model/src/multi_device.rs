//! Authority-free multi-device peer topology and transfer lifecycle model.
//!
//! The model consumes caller-constructed topology observations and existing
//! device/memory lifecycle states. It grants no native peer enablement, VMM,
//! copy, dispatch, completion, visibility, or hardware authority.
//! V1 admits exactly two mapped devices and binds the accessing device for
//! each region, but the underlying memory model still applies one uniform
//! access mode to the complete mapping device set.

use alloc::{boxed::Box, vec::Vec};

use crate::*;

pub const MULTI_DEVICE_MODEL_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_PEER_TRANSFER_RECORDS_V1: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PeerTopologyIdV1(IdentityDigestV1);

impl PeerTopologyIdV1 {
    pub const fn from_untrusted_digest(digest: IdentityDigestV1) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> IdentityDigestV1 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PeerTransferCompletionIdV1(IdentityDigestV1);

impl PeerTransferCompletionIdV1 {
    pub const fn from_untrusted_digest(digest: IdentityDigestV1) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> IdentityDigestV1 {
        self.0
    }
}

/// Caller-constructed projection of a directional HIP/VMM capability query.
///
/// A native adapter must independently authenticate every field and retain any
/// concrete enablement owner. This record only allows the pure model to reject
/// inconsistent device, VM, profile, generation, and observation coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedPeerTopologyObservationV1 {
    pub schema_version: u16,
    pub domain_id: DeviceObservationDomainIdV1,
    pub topology_id: PeerTopologyIdV1,
    pub observation_epoch: ObservationEpochV1,
    pub source_device: DeviceKeyV1,
    pub destination_device: DeviceKeyV1,
    pub source_profile: DeviceAdmissionProfileIdV1,
    pub destination_profile: DeviceAdmissionProfileIdV1,
    pub source_vm: VmKeyV1,
    pub destination_vm: VmKeyV1,
    pub peer_access_supported: bool,
    pub virtual_memory_management_supported: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerTopologyAdmissionErrorV1 {
    InvalidSchema,
    InvalidIdentity,
    ObservationDomainMismatch,
    ObservationEpochMismatch,
    SameDevice,
    PeerAccessUnavailable,
    VirtualMemoryManagementUnavailable,
    SourceDeviceNotCurrent,
    DestinationDeviceNotCurrent,
    DeviceProfileMismatch,
    VmNotCurrent,
    VmBindingMismatch,
    InvalidIdentityState,
    InvalidMemoryState,
    MemoryVmMismatch,
    DeviceSetMismatch,
    TopologyIdentityMismatch,
}

/// Exact directional peer relationship admitted by the executable model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelPeerTopologyV1 {
    domain_id: DeviceObservationDomainIdV1,
    topology_id: PeerTopologyIdV1,
    observation_epoch: ObservationEpochV1,
    source_device: DeviceKeyV1,
    destination_device: DeviceKeyV1,
    source_correlation: ModelCorrelatedDeviceV1,
    destination_correlation: ModelCorrelatedDeviceV1,
    source_profile: DeviceAdmissionProfileIdV1,
    destination_profile: DeviceAdmissionProfileIdV1,
    source_vm: VmKeyV1,
    destination_vm: VmKeyV1,
}

impl ModelPeerTopologyV1 {
    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn domain_id(self) -> DeviceObservationDomainIdV1 {
        self.domain_id
    }

    pub const fn topology_id(self) -> PeerTopologyIdV1 {
        self.topology_id
    }

    pub const fn observation_epoch(self) -> ObservationEpochV1 {
        self.observation_epoch
    }

    pub const fn source_device(self) -> DeviceKeyV1 {
        self.source_device
    }

    pub const fn destination_device(self) -> DeviceKeyV1 {
        self.destination_device
    }

    pub const fn source_correlation(self) -> ModelCorrelatedDeviceV1 {
        self.source_correlation
    }

    pub const fn destination_correlation(self) -> ModelCorrelatedDeviceV1 {
        self.destination_correlation
    }

    pub const fn source_profile(self) -> DeviceAdmissionProfileIdV1 {
        self.source_profile
    }

    pub const fn destination_profile(self) -> DeviceAdmissionProfileIdV1 {
        self.destination_profile
    }

    pub const fn source_vm(self) -> VmKeyV1 {
        self.source_vm
    }

    pub const fn destination_vm(self) -> VmKeyV1 {
        self.destination_vm
    }
}

pub fn admit_peer_topology_model_only_v1(
    identity: &DeviceIdentityStateV1,
    memory: &MemoryLifecycleStateV1,
    observation: UntrustedPeerTopologyObservationV1,
) -> Result<ModelPeerTopologyV1, PeerTopologyAdmissionErrorV1> {
    identity
        .validate_global_invariants()
        .map_err(|_| PeerTopologyAdmissionErrorV1::InvalidIdentityState)?;
    memory
        .validate_global_invariants()
        .map_err(|_| PeerTopologyAdmissionErrorV1::InvalidMemoryState)?;
    if observation.schema_version != MULTI_DEVICE_MODEL_SCHEMA_VERSION_V1 {
        return Err(PeerTopologyAdmissionErrorV1::InvalidSchema);
    }
    if digest_is_zero(observation.topology_id.digest())
        || observation.observation_epoch.0 == 0
        || observation.source_device.generation.0 == 0
        || observation.destination_device.generation.0 == 0
        || observation.source_vm.id.0 == 0
        || observation.destination_vm.id.0 == 0
        || digest_is_zero(observation.source_profile.digest())
        || digest_is_zero(observation.destination_profile.digest())
    {
        return Err(PeerTopologyAdmissionErrorV1::InvalidIdentity);
    }
    if identity.domain_id() != observation.domain_id || memory.domain_id() != observation.domain_id
    {
        return Err(PeerTopologyAdmissionErrorV1::ObservationDomainMismatch);
    }
    if observation.source_device.physical == observation.destination_device.physical {
        return Err(PeerTopologyAdmissionErrorV1::SameDevice);
    }
    if !observation.peer_access_supported {
        return Err(PeerTopologyAdmissionErrorV1::PeerAccessUnavailable);
    }
    if !observation.virtual_memory_management_supported {
        return Err(PeerTopologyAdmissionErrorV1::VirtualMemoryManagementUnavailable);
    }
    if observation.source_vm.device != observation.source_device
        || observation.destination_vm.device != observation.destination_device
        || observation.source_vm == observation.destination_vm
    {
        return Err(PeerTopologyAdmissionErrorV1::VmBindingMismatch);
    }
    let source = current_device_record(identity, observation.source_device)
        .ok_or(PeerTopologyAdmissionErrorV1::SourceDeviceNotCurrent)?;
    let destination = current_device_record(identity, observation.destination_device)
        .ok_or(PeerTopologyAdmissionErrorV1::DestinationDeviceNotCurrent)?;
    if source.profile_id != observation.source_profile
        || destination.profile_id != observation.destination_profile
    {
        return Err(PeerTopologyAdmissionErrorV1::DeviceProfileMismatch);
    }
    if source.correlation.epoch() != observation.observation_epoch
        || destination.correlation.epoch() != observation.observation_epoch
    {
        return Err(PeerTopologyAdmissionErrorV1::ObservationEpochMismatch);
    }
    for vm in [observation.source_vm, observation.destination_vm] {
        if !identity
            .vms()
            .iter()
            .any(|record| record.key == vm && record.status == ModelAdmissionStatusV1::Active)
        {
            return Err(PeerTopologyAdmissionErrorV1::VmNotCurrent);
        }
    }
    let expected_devices =
        canonical_peer_devices(observation.source_device, observation.destination_device);
    for vm in [observation.source_vm, observation.destination_vm] {
        let Some(record) = memory.vms().iter().find(|record| {
            record.admission.model_key() == vm && record.state == MemoryVmStateV1::Active
        }) else {
            return Err(PeerTopologyAdmissionErrorV1::MemoryVmMismatch);
        };
        let actual: Vec<_> = record.mapping_device_keys().collect();
        if actual.as_slice() != expected_devices {
            return Err(PeerTopologyAdmissionErrorV1::DeviceSetMismatch);
        }
        if record
            .mapping_devices
            .iter()
            .any(|admission| !device_admission_is_current(identity, *admission))
        {
            return Err(PeerTopologyAdmissionErrorV1::MemoryVmMismatch);
        }
    }
    Ok(ModelPeerTopologyV1 {
        domain_id: observation.domain_id,
        topology_id: observation.topology_id,
        observation_epoch: observation.observation_epoch,
        source_device: observation.source_device,
        destination_device: observation.destination_device,
        source_correlation: source.correlation,
        destination_correlation: destination.correlation,
        source_profile: observation.source_profile,
        destination_profile: observation.destination_profile,
        source_vm: observation.source_vm,
        destination_vm: observation.destination_vm,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerTransferMechanismV1 {
    /// Declared external copy contract. Authentication and implementation are
    /// deliberately outside this model.
    DeclaredPeerCopy { contract_identity: IdentityDigestV1 },
}

/// Allocation-relative region and the exact modeled device access it requires.
///
/// `accessing_device` is an identity coordinate, not a native VMM access
/// receipt. The containing mapping supplies one uniform modeled access mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerTransferRegionV1 {
    mapping: MemoryMappingKeyV1,
    owning_device: DeviceKeyV1,
    accessing_device: DeviceKeyV1,
    required_access: MemoryAccessV1,
    byte_offset: u64,
}

impl PeerTransferRegionV1 {
    pub const fn new(
        mapping: MemoryMappingKeyV1,
        owning_device: DeviceKeyV1,
        accessing_device: DeviceKeyV1,
        required_access: MemoryAccessV1,
        byte_offset: u64,
    ) -> Self {
        Self {
            mapping,
            owning_device,
            accessing_device,
            required_access,
            byte_offset,
        }
    }

    pub const fn mapping(self) -> MemoryMappingKeyV1 {
        self.mapping
    }

    pub const fn owning_device(self) -> DeviceKeyV1 {
        self.owning_device
    }

    pub const fn accessing_device(self) -> DeviceKeyV1 {
        self.accessing_device
    }

    pub const fn required_access(self) -> MemoryAccessV1 {
        self.required_access
    }

    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerTransferRequestV1 {
    transfer_id: u64,
    topology_id: PeerTopologyIdV1,
    mechanism: PeerTransferMechanismV1,
    source: PeerTransferRegionV1,
    destination: PeerTransferRegionV1,
    byte_len: u64,
    required_alignment: u64,
}

impl PeerTransferRequestV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        transfer_id: u64,
        topology_id: PeerTopologyIdV1,
        mechanism: PeerTransferMechanismV1,
        source: PeerTransferRegionV1,
        destination: PeerTransferRegionV1,
        byte_len: u64,
        required_alignment: u64,
    ) -> Self {
        Self {
            transfer_id,
            topology_id,
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

    pub const fn topology_id(self) -> PeerTopologyIdV1 {
        self.topology_id
    }

    pub const fn mechanism(self) -> PeerTransferMechanismV1 {
        self.mechanism
    }

    pub const fn source(self) -> PeerTransferRegionV1 {
        self.source
    }

    pub const fn destination(self) -> PeerTransferRegionV1 {
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
pub enum PeerTransferAdmissionErrorV1 {
    InvalidIdentity,
    TopologyMismatch,
    EndpointBindingMismatch,
    AliasedEndpoints,
    InvalidMemoryState,
    MappingNotLive,
    AllocationNotLive,
    UnsupportedMemoryKind,
    InvalidAccess,
    InvalidRange,
    InvalidAlignment,
    DeviceSetMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerTransferRetentionV1 {
    source: MemoryPublicationKeyV1,
    destination: MemoryPublicationKeyV1,
}

impl PeerTransferRetentionV1 {
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

/// Structural memory-publication owner for one exact modeled peer transfer.
///
/// This identity prevents generic or different-transfer model transitions from
/// discharging retained mappings. It grants no native memory or peer authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerTransferPublicationOwnerV1 {
    registry_incarnation: IdentityDigestV1,
    transfer_id: u64,
    topology_id: PeerTopologyIdV1,
    completion: PeerTransferCompletionIdV1,
}

impl PeerTransferPublicationOwnerV1 {
    pub const fn registry_incarnation(self) -> IdentityDigestV1 {
        self.registry_incarnation
    }

    pub const fn transfer_id(self) -> u64 {
        self.transfer_id
    }

    pub const fn topology_id(self) -> PeerTopologyIdV1 {
        self.topology_id
    }

    pub const fn completion(self) -> PeerTransferCompletionIdV1 {
        self.completion
    }

    pub(crate) fn has_valid_identity(self) -> bool {
        self.transfer_id != 0
            && !digest_is_zero(self.registry_incarnation)
            && !digest_is_zero(self.topology_id.digest())
            && !digest_is_zero(self.completion.digest())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerTransferBindingV1 {
    registry_incarnation: IdentityDigestV1,
    transfer_id: u64,
    topology: ModelPeerTopologyV1,
    completion: PeerTransferCompletionIdV1,
    retention: PeerTransferRetentionV1,
}

impl PeerTransferBindingV1 {
    pub const fn registry_incarnation(self) -> IdentityDigestV1 {
        self.registry_incarnation
    }

    pub const fn transfer_id(self) -> u64 {
        self.transfer_id
    }

    pub const fn topology_id(self) -> PeerTopologyIdV1 {
        self.topology.topology_id
    }

    pub const fn topology(self) -> ModelPeerTopologyV1 {
        self.topology
    }

    pub const fn completion(self) -> PeerTransferCompletionIdV1 {
        self.completion
    }

    pub const fn retention(self) -> PeerTransferRetentionV1 {
        self.retention
    }

    pub const fn publication_owner(self) -> PeerTransferPublicationOwnerV1 {
        PeerTransferPublicationOwnerV1 {
            registry_incarnation: self.registry_incarnation,
            transfer_id: self.transfer_id,
            topology_id: self.topology.topology_id,
            completion: self.completion,
        }
    }

    pub(crate) fn retains_publication(self, key: MemoryPublicationKeyV1) -> bool {
        (key.id.0 != 0)
            && (key == self.retention.source || key == self.retention.destination)
            && self.retention.source != self.retention.destination
            && self.transfer_id != 0
            && !digest_is_zero(self.registry_incarnation)
            && !digest_is_zero(self.topology.topology_id.digest())
            && !digest_is_zero(self.completion.digest())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerTransferPhaseV1 {
    Reserved,
    Published { submission_sequence: u64 },
    VisibilityObserved { acquire_sequence: u64 },
    Indeterminate,
    Released,
}

impl PeerTransferPhaseV1 {
    const fn retains_memory(self) -> bool {
        !matches!(self, Self::Released)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PeerTransferRecordV1 {
    binding: PeerTransferBindingV1,
    request: PeerTransferRequestV1,
    phase: PeerTransferPhaseV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerTransferErrorV1 {
    InvalidRegistryIncarnation,
    CapacityExceeded,
    Topology(PeerTopologyAdmissionErrorV1),
    Admission(PeerTransferAdmissionErrorV1),
    InvalidRetention,
    InvalidCompletionIdentity,
    DuplicateIdentity,
    ResourceConflict,
    TokenMismatch,
    InvalidOrdering,
    Memory(MemoryTransitionErrorV1),
}

#[must_use]
pub struct PeerTransferRegistryCreateFailureV1 {
    error: PeerTransferErrorV1,
    identity: Box<DeviceIdentityStateV1>,
    memory: Box<MemoryLifecycleStateV1>,
}

impl PeerTransferRegistryCreateFailureV1 {
    pub const fn error(&self) -> PeerTransferErrorV1 {
        self.error
    }

    pub fn into_states(self) -> (DeviceIdentityStateV1, MemoryLifecycleStateV1) {
        (*self.identity, *self.memory)
    }
}

impl core::fmt::Debug for PeerTransferRegistryCreateFailureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PeerTransferRegistryCreateFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

pub struct PeerTransferRegistryV1 {
    identity: DeviceIdentityStateV1,
    memory: MemoryLifecycleStateV1,
    topology: ModelPeerTopologyV1,
    registry_incarnation: IdentityDigestV1,
    records: Vec<PeerTransferRecordV1>,
}

impl core::fmt::Debug for PeerTransferRegistryV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PeerTransferRegistryV1")
            .field("topology", &self.topology)
            .field("registry_incarnation", &self.registry_incarnation)
            .field("records", &self.records)
            .finish_non_exhaustive()
    }
}

impl PeerTransferRegistryV1 {
    pub fn new_model_only(
        identity: DeviceIdentityStateV1,
        memory: MemoryLifecycleStateV1,
        topology: ModelPeerTopologyV1,
        current_observation: UntrustedPeerTopologyObservationV1,
        registry_incarnation: IdentityDigestV1,
    ) -> Result<Self, PeerTransferRegistryCreateFailureV1> {
        let error = if digest_is_zero(registry_incarnation) {
            Some(PeerTransferErrorV1::InvalidRegistryIncarnation)
        } else {
            validate_peer_topology_current(&identity, &memory, topology, current_observation)
                .err()
                .map(PeerTransferErrorV1::Topology)
        };
        if let Some(error) = error {
            return Err(PeerTransferRegistryCreateFailureV1 {
                error,
                identity: Box::new(identity),
                memory: Box::new(memory),
            });
        }
        Ok(Self {
            identity,
            memory,
            topology,
            registry_incarnation,
            records: Vec::new(),
        })
    }

    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn topology(&self) -> ModelPeerTopologyV1 {
        self.topology
    }

    pub fn identity_state(&self) -> &DeviceIdentityStateV1 {
        &self.identity
    }

    pub fn memory_state(&self) -> &MemoryLifecycleStateV1 {
        &self.memory
    }

    pub fn retained_transfer_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| record.phase.retains_memory())
            .count()
    }

    pub fn into_states(self) -> Result<(DeviceIdentityStateV1, MemoryLifecycleStateV1), Box<Self>> {
        if self.retained_transfer_count() == 0 {
            Ok((self.identity, self.memory))
        } else {
            Err(Box::new(self))
        }
    }

    pub fn reserve_model_only(
        &mut self,
        identity: &DeviceIdentityStateV1,
        current_observation: UntrustedPeerTopologyObservationV1,
        request: PeerTransferRequestV1,
        completion: PeerTransferCompletionIdV1,
        retention: PeerTransferRetentionV1,
    ) -> Result<PeerTransferReservedTokenV1, PeerTransferErrorV1> {
        if self.records.len() >= MAX_PEER_TRANSFER_RECORDS_V1 {
            return Err(PeerTransferErrorV1::CapacityExceeded);
        }
        self.validate_context(identity, current_observation)?;
        validate_peer_transfer_request(&self.memory, self.topology, request)
            .map_err(PeerTransferErrorV1::Admission)?;
        if digest_is_zero(completion.digest()) {
            return Err(PeerTransferErrorV1::InvalidCompletionIdentity);
        }
        if retention.source.mapping != request.source.mapping
            || retention.destination.mapping != request.destination.mapping
            || retention.source.id.0 == 0
            || retention.destination.id.0 == 0
            || retention.source == retention.destination
        {
            return Err(PeerTransferErrorV1::InvalidRetention);
        }
        if self.records.iter().any(|record| {
            record.binding.transfer_id == request.transfer_id
                || record.binding.completion == completion
                || record.binding.retention.source == retention.source
                || record.binding.retention.destination == retention.destination
        }) {
            return Err(PeerTransferErrorV1::DuplicateIdentity);
        }
        if self.records.iter().any(|record| {
            record.phase.retains_memory()
                && peer_transfer_requests_conflict(record.request, request)
        }) {
            return Err(PeerTransferErrorV1::ResourceConflict);
        }
        let binding = PeerTransferBindingV1 {
            registry_incarnation: self.registry_incarnation,
            transfer_id: request.transfer_id,
            topology: self.topology,
            completion,
            retention,
        };
        let source_published = self
            .memory
            .publish_peer_transfer_mapping(retention.source, binding)
            .map_err(PeerTransferErrorV1::Memory)?;
        let retained = source_published
            .publish_peer_transfer_mapping(retention.destination, binding)
            .map_err(PeerTransferErrorV1::Memory)?;
        self.identity = identity.clone();
        self.memory = retained;
        self.records.push(PeerTransferRecordV1 {
            binding,
            request,
            phase: PeerTransferPhaseV1::Reserved,
        });
        Ok(PeerTransferReservedTokenV1 { binding, request })
    }

    fn validate_context(
        &self,
        identity: &DeviceIdentityStateV1,
        current_observation: UntrustedPeerTopologyObservationV1,
    ) -> Result<(), PeerTransferErrorV1> {
        validate_peer_topology_current(identity, &self.memory, self.topology, current_observation)
            .map_err(PeerTransferErrorV1::Topology)
    }

    fn record_index(
        &self,
        binding: PeerTransferBindingV1,
        request: PeerTransferRequestV1,
        phase: PeerTransferPhaseV1,
    ) -> Result<usize, PeerTransferErrorV1> {
        if binding.registry_incarnation != self.registry_incarnation {
            return Err(PeerTransferErrorV1::TokenMismatch);
        }
        self.records
            .iter()
            .position(|record| {
                record.binding == binding && record.request == request && record.phase == phase
            })
            .ok_or(PeerTransferErrorV1::TokenMismatch)
    }

    fn release_retention(
        &self,
        binding: PeerTransferBindingV1,
    ) -> Result<MemoryLifecycleStateV1, PeerTransferErrorV1> {
        let retention = binding.retention;
        let source_released = self
            .memory
            .release_peer_transfer_publication(retention.source, binding)
            .map_err(PeerTransferErrorV1::Memory)?;
        source_released
            .release_peer_transfer_publication(retention.destination, binding)
            .map_err(PeerTransferErrorV1::Memory)
    }

    fn publish(
        &mut self,
        identity: &DeviceIdentityStateV1,
        current_observation: UntrustedPeerTopologyObservationV1,
        binding: PeerTransferBindingV1,
        request: PeerTransferRequestV1,
        submission_sequence: u64,
    ) -> Result<(), PeerTransferErrorV1> {
        let index = self.record_index(binding, request, PeerTransferPhaseV1::Reserved)?;
        self.validate_context(identity, current_observation)?;
        if submission_sequence == 0 {
            return Err(PeerTransferErrorV1::InvalidOrdering);
        }
        self.identity = identity.clone();
        self.records[index].phase = PeerTransferPhaseV1::Published {
            submission_sequence,
        };
        Ok(())
    }

    fn cancel_reserved(
        &mut self,
        binding: PeerTransferBindingV1,
        request: PeerTransferRequestV1,
    ) -> Result<(), PeerTransferErrorV1> {
        let index = self.record_index(binding, request, PeerTransferPhaseV1::Reserved)?;
        let memory = self.release_retention(binding)?;
        self.memory = memory;
        self.records[index].phase = PeerTransferPhaseV1::Released;
        Ok(())
    }

    fn poll(
        &mut self,
        identity: &DeviceIdentityStateV1,
        current_observation: UntrustedPeerTopologyObservationV1,
        binding: PeerTransferBindingV1,
        request: PeerTransferRequestV1,
        submission_sequence: u64,
        observation: PeerTransferCompletionObservationV1,
    ) -> Result<PeerTransferPollTransitionV1, PeerTransferErrorV1> {
        let index = self.record_index(
            binding,
            request,
            PeerTransferPhaseV1::Published {
                submission_sequence,
            },
        )?;
        match observation {
            PeerTransferCompletionObservationV1::Pending => {
                self.validate_context(identity, current_observation)?;
                self.identity = identity.clone();
                Ok(PeerTransferPollTransitionV1::Pending)
            }
            PeerTransferCompletionObservationV1::Completed {
                completion,
                acquire_sequence,
            } => {
                self.validate_context(identity, current_observation)?;
                if completion != binding.completion {
                    return Err(PeerTransferErrorV1::TokenMismatch);
                }
                if acquire_sequence <= submission_sequence {
                    return Err(PeerTransferErrorV1::InvalidOrdering);
                }
                self.identity = identity.clone();
                self.records[index].phase =
                    PeerTransferPhaseV1::VisibilityObserved { acquire_sequence };
                Ok(PeerTransferPollTransitionV1::Completed { acquire_sequence })
            }
            PeerTransferCompletionObservationV1::Indeterminate => {
                self.records[index].phase = PeerTransferPhaseV1::Indeterminate;
                Ok(PeerTransferPollTransitionV1::Indeterminate)
            }
        }
    }

    fn release_visibility(
        &mut self,
        identity: &DeviceIdentityStateV1,
        current_observation: UntrustedPeerTopologyObservationV1,
        binding: PeerTransferBindingV1,
        request: PeerTransferRequestV1,
        acquire_sequence: u64,
    ) -> Result<(), PeerTransferErrorV1> {
        let index = self.record_index(
            binding,
            request,
            PeerTransferPhaseV1::VisibilityObserved { acquire_sequence },
        )?;
        self.validate_context(identity, current_observation)?;
        let memory = self.release_retention(binding)?;
        self.identity = identity.clone();
        self.memory = memory;
        self.records[index].phase = PeerTransferPhaseV1::Released;
        Ok(())
    }

    fn quarantine_currentness_loss(
        &mut self,
        binding: PeerTransferBindingV1,
        request: PeerTransferRequestV1,
        phase: PeerTransferPhaseV1,
    ) -> Result<(), PeerTransferErrorV1> {
        let index = self.record_index(binding, request, phase)?;
        self.records[index].phase = PeerTransferPhaseV1::Indeterminate;
        Ok(())
    }
}

#[must_use]
pub struct PeerTransferTokenFailureV1<T> {
    error: PeerTransferErrorV1,
    retained: Box<T>,
}

impl<T> PeerTransferTokenFailureV1<T> {
    pub const fn error(&self) -> PeerTransferErrorV1 {
        self.error
    }

    pub fn into_retained(self) -> T {
        *self.retained
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for PeerTransferTokenFailureV1<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PeerTransferTokenFailureV1")
            .field("error", &self.error)
            .field("retained", &self.retained)
            .finish()
    }
}

#[derive(Debug)]
#[must_use = "a peer transfer reservation must be published or cancelled"]
pub struct PeerTransferReservedTokenV1 {
    binding: PeerTransferBindingV1,
    request: PeerTransferRequestV1,
}

impl PeerTransferReservedTokenV1 {
    pub const fn binding(&self) -> PeerTransferBindingV1 {
        self.binding
    }

    pub const fn request(&self) -> PeerTransferRequestV1 {
        self.request
    }

    pub fn publish_model_only(
        self,
        registry: &mut PeerTransferRegistryV1,
        identity: &DeviceIdentityStateV1,
        current_observation: UntrustedPeerTopologyObservationV1,
        submission_sequence: u64,
    ) -> Result<PeerTransferPublishedTokenV1, PeerTransferTokenFailureV1<Self>> {
        match registry.publish(
            identity,
            current_observation,
            self.binding,
            self.request,
            submission_sequence,
        ) {
            Ok(()) => Ok(PeerTransferPublishedTokenV1 {
                binding: self.binding,
                request: self.request,
                submission_sequence,
            }),
            Err(error) => Err(PeerTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }

    pub fn cancel_before_publication_model_only(
        self,
        registry: &mut PeerTransferRegistryV1,
    ) -> Result<PeerTransferReleasedReceiptV1, PeerTransferTokenFailureV1<Self>> {
        match registry.cancel_reserved(self.binding, self.request) {
            Ok(()) => Ok(PeerTransferReleasedReceiptV1 {
                binding: self.binding,
                request: self.request,
                acquire_sequence: None,
            }),
            Err(error) => Err(PeerTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeerTransferCompletionObservationV1 {
    Pending,
    Completed {
        completion: PeerTransferCompletionIdV1,
        acquire_sequence: u64,
    },
    Indeterminate,
}

#[derive(Debug)]
#[must_use = "published peer transfer custody must reach a terminal observation"]
pub struct PeerTransferPublishedTokenV1 {
    binding: PeerTransferBindingV1,
    request: PeerTransferRequestV1,
    submission_sequence: u64,
}

impl PeerTransferPublishedTokenV1 {
    pub const fn binding(&self) -> PeerTransferBindingV1 {
        self.binding
    }

    pub const fn request(&self) -> PeerTransferRequestV1 {
        self.request
    }

    pub fn poll_model_only(
        self,
        registry: &mut PeerTransferRegistryV1,
        identity: &DeviceIdentityStateV1,
        current_observation: UntrustedPeerTopologyObservationV1,
        observation: PeerTransferCompletionObservationV1,
    ) -> Result<PeerTransferPollV1, PeerTransferTokenFailureV1<Self>> {
        match registry.poll(
            identity,
            current_observation,
            self.binding,
            self.request,
            self.submission_sequence,
            observation,
        ) {
            Ok(PeerTransferPollTransitionV1::Pending) => Ok(PeerTransferPollV1::Pending(self)),
            Ok(PeerTransferPollTransitionV1::Completed { acquire_sequence }) => Ok(
                PeerTransferPollV1::Completed(PeerTransferVisibilityTokenV1 {
                    binding: self.binding,
                    request: self.request,
                    acquire_sequence,
                }),
            ),
            Ok(PeerTransferPollTransitionV1::Indeterminate) => Ok(
                PeerTransferPollV1::Indeterminate(PeerTransferQuarantineV1 {
                    binding: self.binding,
                    request: self.request,
                }),
            ),
            Err(error) => Err(PeerTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }

    /// Conservatively records that an already-published transfer can no
    /// longer be related to a current device/topology observation.
    pub fn quarantine_currentness_loss_model_only(
        self,
        registry: &mut PeerTransferRegistryV1,
    ) -> Result<PeerTransferQuarantineV1, PeerTransferTokenFailureV1<Self>> {
        let phase = PeerTransferPhaseV1::Published {
            submission_sequence: self.submission_sequence,
        };
        match registry.quarantine_currentness_loss(self.binding, self.request, phase) {
            Ok(()) => Ok(PeerTransferQuarantineV1 {
                binding: self.binding,
                request: self.request,
            }),
            Err(error) => Err(PeerTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }
}

enum PeerTransferPollTransitionV1 {
    Pending,
    Completed { acquire_sequence: u64 },
    Indeterminate,
}

#[derive(Debug)]
pub enum PeerTransferPollV1 {
    Pending(PeerTransferPublishedTokenV1),
    Completed(PeerTransferVisibilityTokenV1),
    Indeterminate(PeerTransferQuarantineV1),
}

#[derive(Debug)]
#[must_use = "peer destination visibility retains both mappings until consumed"]
pub struct PeerTransferVisibilityTokenV1 {
    binding: PeerTransferBindingV1,
    request: PeerTransferRequestV1,
    acquire_sequence: u64,
}

impl PeerTransferVisibilityTokenV1 {
    pub const fn binding(&self) -> PeerTransferBindingV1 {
        self.binding
    }

    pub const fn request(&self) -> PeerTransferRequestV1 {
        self.request
    }

    pub const fn acquire_sequence(&self) -> u64 {
        self.acquire_sequence
    }

    pub const fn visible_device(&self) -> DeviceKeyV1 {
        self.request.destination.owning_device
    }

    pub fn release_after_visibility_consumed_model_only(
        self,
        registry: &mut PeerTransferRegistryV1,
        identity: &DeviceIdentityStateV1,
        current_observation: UntrustedPeerTopologyObservationV1,
    ) -> Result<PeerTransferReleasedReceiptV1, PeerTransferTokenFailureV1<Self>> {
        match registry.release_visibility(
            identity,
            current_observation,
            self.binding,
            self.request,
            self.acquire_sequence,
        ) {
            Ok(()) => Ok(PeerTransferReleasedReceiptV1 {
                binding: self.binding,
                request: self.request,
                acquire_sequence: Some(self.acquire_sequence),
            }),
            Err(error) => Err(PeerTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }

    /// Conservatively retains both mappings when visibility was observed but
    /// the current topology can no longer be revalidated before consumption.
    pub fn quarantine_currentness_loss_model_only(
        self,
        registry: &mut PeerTransferRegistryV1,
    ) -> Result<PeerTransferQuarantineV1, PeerTransferTokenFailureV1<Self>> {
        let phase = PeerTransferPhaseV1::VisibilityObserved {
            acquire_sequence: self.acquire_sequence,
        };
        match registry.quarantine_currentness_loss(self.binding, self.request, phase) {
            Ok(()) => Ok(PeerTransferQuarantineV1 {
                binding: self.binding,
                request: self.request,
            }),
            Err(error) => Err(PeerTransferTokenFailureV1 {
                error,
                retained: Box::new(self),
            }),
        }
    }
}

#[derive(Debug)]
#[must_use = "indeterminate peer transfer custody has no release transition"]
pub struct PeerTransferQuarantineV1 {
    binding: PeerTransferBindingV1,
    request: PeerTransferRequestV1,
}

impl PeerTransferQuarantineV1 {
    pub const fn binding(&self) -> PeerTransferBindingV1 {
        self.binding
    }

    pub const fn request(&self) -> PeerTransferRequestV1 {
        self.request
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerTransferReleasedReceiptV1 {
    binding: PeerTransferBindingV1,
    request: PeerTransferRequestV1,
    acquire_sequence: Option<u64>,
}

impl PeerTransferReleasedReceiptV1 {
    pub const fn binding(self) -> PeerTransferBindingV1 {
        self.binding
    }

    pub const fn request(self) -> PeerTransferRequestV1 {
        self.request
    }

    pub const fn acquire_sequence(self) -> Option<u64> {
        self.acquire_sequence
    }
}

fn validate_peer_topology_current(
    identity: &DeviceIdentityStateV1,
    memory: &MemoryLifecycleStateV1,
    topology: ModelPeerTopologyV1,
    current_observation: UntrustedPeerTopologyObservationV1,
) -> Result<(), PeerTopologyAdmissionErrorV1> {
    let current = admit_peer_topology_model_only_v1(identity, memory, current_observation)?;
    if current != topology {
        return Err(PeerTopologyAdmissionErrorV1::TopologyIdentityMismatch);
    }
    Ok(())
}

fn validate_peer_transfer_request(
    memory: &MemoryLifecycleStateV1,
    topology: ModelPeerTopologyV1,
    request: PeerTransferRequestV1,
) -> Result<(), PeerTransferAdmissionErrorV1> {
    memory
        .validate_global_invariants()
        .map_err(|_| PeerTransferAdmissionErrorV1::InvalidMemoryState)?;
    if request.transfer_id == 0 || mechanism_identity_is_zero(request.mechanism) {
        return Err(PeerTransferAdmissionErrorV1::InvalidIdentity);
    }
    if request.topology_id != topology.topology_id {
        return Err(PeerTransferAdmissionErrorV1::TopologyMismatch);
    }
    if request.byte_len == 0 {
        return Err(PeerTransferAdmissionErrorV1::InvalidRange);
    }
    if request.required_alignment == 0 || !request.required_alignment.is_power_of_two() {
        return Err(PeerTransferAdmissionErrorV1::InvalidAlignment);
    }
    if request.source.mapping.allocation == request.destination.mapping.allocation {
        return Err(PeerTransferAdmissionErrorV1::AliasedEndpoints);
    }
    if request.source.owning_device != topology.source_device
        || request.destination.owning_device != topology.destination_device
        || request.source.accessing_device != topology.source_device
        || request.destination.accessing_device != topology.source_device
        || request.source.required_access != MemoryAccessV1::Read
        || request.destination.required_access != MemoryAccessV1::ReadWrite
        || request.source.mapping.allocation.vm != topology.source_vm
        || request.destination.mapping.allocation.vm != topology.destination_vm
    {
        return Err(PeerTransferAdmissionErrorV1::EndpointBindingMismatch);
    }
    let expected_devices =
        canonical_peer_devices(topology.source_device, topology.destination_device);
    validate_peer_region(
        memory,
        request.source,
        request.byte_len,
        request.required_alignment,
        expected_devices,
    )?;
    validate_peer_region(
        memory,
        request.destination,
        request.byte_len,
        request.required_alignment,
        expected_devices,
    )?;
    Ok(())
}

fn validate_peer_region(
    memory: &MemoryLifecycleStateV1,
    region: PeerTransferRegionV1,
    byte_len: u64,
    alignment: u64,
    expected_devices: [DeviceKeyV1; 2],
) -> Result<(), PeerTransferAdmissionErrorV1> {
    let mapping = memory
        .mappings()
        .iter()
        .find(|record| record.key == region.mapping)
        .ok_or(PeerTransferAdmissionErrorV1::MappingNotLive)?;
    if mapping.state != MemoryMappingStateV1::Mapped {
        return Err(PeerTransferAdmissionErrorV1::MappingNotLive);
    }
    if mapping.target_devices.as_slice() != expected_devices {
        return Err(PeerTransferAdmissionErrorV1::DeviceSetMismatch);
    }
    if !mapping.target_devices.contains(&region.accessing_device)
        || !mapping.access.permits(region.required_access)
    {
        return Err(PeerTransferAdmissionErrorV1::InvalidAccess);
    }
    let allocation = memory
        .allocations()
        .iter()
        .find(|record| record.key == region.mapping.allocation)
        .ok_or(PeerTransferAdmissionErrorV1::AllocationNotLive)?;
    if allocation.state != MemoryAllocationStateV1::Live {
        return Err(PeerTransferAdmissionErrorV1::AllocationNotLive);
    }
    if allocation.key.vm.device != region.owning_device {
        return Err(PeerTransferAdmissionErrorV1::EndpointBindingMismatch);
    }
    if allocation.spec.kind != MemoryKindV1::DeviceLocal
        || allocation.spec.coherence != MemoryCoherenceV1::ExplicitVisibility
    {
        return Err(PeerTransferAdmissionErrorV1::UnsupportedMemoryKind);
    }
    if allocation.spec.alignment < alignment || !region.byte_offset.is_multiple_of(alignment) {
        return Err(PeerTransferAdmissionErrorV1::InvalidAlignment);
    }
    if region
        .byte_offset
        .checked_add(byte_len)
        .is_none_or(|end| end > allocation.spec.byte_len)
    {
        return Err(PeerTransferAdmissionErrorV1::InvalidRange);
    }
    Ok(())
}

fn peer_transfer_requests_conflict(
    left: PeerTransferRequestV1,
    right: PeerTransferRequestV1,
) -> bool {
    let left_regions = [(left.source, false), (left.destination, true)];
    let right_regions = [(right.source, false), (right.destination, true)];
    left_regions.iter().any(|(left_region, left_write)| {
        right_regions.iter().any(|(right_region, right_write)| {
            (*left_write || *right_write)
                && peer_regions_overlap(*left_region, left.byte_len, *right_region, right.byte_len)
        })
    })
}

fn peer_regions_overlap(
    left: PeerTransferRegionV1,
    left_len: u64,
    right: PeerTransferRegionV1,
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

fn current_device_record(
    identity: &DeviceIdentityStateV1,
    key: DeviceKeyV1,
) -> Option<&ModelDeviceAdmissionRecordV1> {
    identity
        .devices()
        .iter()
        .find(|record| record.key == key && record.status == ModelAdmissionStatusV1::Active)
}

fn device_admission_is_current(
    identity: &DeviceIdentityStateV1,
    admission: ModelDeviceAdmissionV1,
) -> bool {
    current_device_record(identity, admission.model_key()).is_some_and(|record| {
        admission.domain_id() == record.domain_id
            && admission.correlation() == record.correlation
            && admission.correlation().profile_id() == record.profile_id
    })
}

fn canonical_peer_devices(source: DeviceKeyV1, destination: DeviceKeyV1) -> [DeviceKeyV1; 2] {
    if source < destination {
        [source, destination]
    } else {
        [destination, source]
    }
}

fn mechanism_identity_is_zero(mechanism: PeerTransferMechanismV1) -> bool {
    match mechanism {
        PeerTransferMechanismV1::DeclaredPeerCopy { contract_identity } => {
            digest_is_zero(contract_identity)
        }
    }
}

fn digest_is_zero(digest: IdentityDigestV1) -> bool {
    digest.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
}

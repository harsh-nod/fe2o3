//! Model-only R9 admission for native XGMI and exact machine-code evidence.
//!
//! The types in this module are authority-free. They check canonical device
//! arrays, cumulative prefix accounting, directional topology coordinates,
//! and equality of caller-supplied code-object evidence. They do not observe
//! KFD, authenticate an attestation, decode instructions, prove compiler
//! correctness, establish coherence, or execute a packet. A native adapter
//! must contract those facts and retain the concrete owners they describe.

use alloc::vec::Vec;

use crate::*;

pub const R9_NATIVE_EVIDENCE_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_NATIVE_MAPPING_DEVICES_V1: usize = 64;
pub const AMDGPU_ELF_MACHINE_V1: u16 = 224;
pub const GFX942_COV6_ABI_VERSION_V1: u8 = 6;
pub const GFX942_COV6_WAVEFRONT_SIZE_V1: u8 = 64;
pub const KFD_XGMI_LINK_TYPE_V1: u32 = 11;
pub const KFD_XGMI_LINK_ENABLED_FLAG_V1: u32 = 1;
pub const GFX942_FIRST_XGMI_SDMA_ENGINE_ID_V1: u32 = 2;
pub const GFX942_SDMA_ENGINE_ID_LIMIT_V1: u32 = 16;

fn identity_digest_is_zero(digest: IdentityDigestV1) -> bool {
    digest.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeMappingPhaseV1 {
    Mapping,
    Active,
    Compensating,
    Compensated,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeProgressStatusV1 {
    Succeeded,
    Failed,
    Indeterminate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UntrustedNativeMultiDeviceMappingV1 {
    pub schema_version: u16,
    pub operation_identity: IdentityDigestV1,
    pub allocation_identity: IdentityDigestV1,
    /// Exact KFD GPU-ID array passed to map and compensation ioctls.
    pub kfd_gpu_ids: Vec<u32>,
}

/// Cumulative-prefix state for one move-only native mapping owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelNativeMultiDeviceMappingV1 {
    operation_identity: IdentityDigestV1,
    allocation_identity: IdentityDigestV1,
    kfd_gpu_ids: Vec<u32>,
    mapped_prefix: usize,
    unmapped_prefix: usize,
    phase: NativeMappingPhaseV1,
}

impl ModelNativeMultiDeviceMappingV1 {
    pub const fn authority_domain(&self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn operation_identity(&self) -> IdentityDigestV1 {
        self.operation_identity
    }

    pub const fn allocation_identity(&self) -> IdentityDigestV1 {
        self.allocation_identity
    }

    pub fn kfd_gpu_ids(&self) -> &[u32] {
        &self.kfd_gpu_ids
    }

    pub const fn mapped_prefix(&self) -> usize {
        self.mapped_prefix
    }

    pub const fn unmapped_prefix(&self) -> usize {
        self.unmapped_prefix
    }

    pub const fn phase(&self) -> NativeMappingPhaseV1 {
        self.phase
    }

    pub const fn is_releasable(&self) -> bool {
        matches!(self.phase, NativeMappingPhaseV1::Compensated)
    }

    pub const fn validate_invariants(&self) -> bool {
        self.mapped_prefix <= self.kfd_gpu_ids.len()
            && self.unmapped_prefix <= self.mapped_prefix
            && match self.phase {
                NativeMappingPhaseV1::Mapping => self.unmapped_prefix == 0,
                NativeMappingPhaseV1::Active => {
                    self.mapped_prefix == self.kfd_gpu_ids.len() && self.unmapped_prefix == 0
                }
                NativeMappingPhaseV1::Compensating => self.unmapped_prefix < self.mapped_prefix,
                NativeMappingPhaseV1::Compensated => self.unmapped_prefix == self.mapped_prefix,
                NativeMappingPhaseV1::Quarantined => true,
            }
    }

    /// Applies the absolute cumulative `n_success` returned by one MAP result.
    pub fn observe_map_cumulative_prefix_model_only_v1(
        mut self,
        submitted_start: usize,
        cumulative_n_success: usize,
        status: NativeProgressStatusV1,
    ) -> Result<Self, NativeMappingAdmissionErrorV1> {
        if !self.validate_invariants() {
            return Err(NativeMappingAdmissionErrorV1::InvalidState);
        }
        if self.phase != NativeMappingPhaseV1::Mapping {
            return Err(NativeMappingAdmissionErrorV1::InvalidPhase);
        }
        if submitted_start != self.mapped_prefix {
            return Err(NativeMappingAdmissionErrorV1::NonCumulativePrefix);
        }
        if cumulative_n_success < submitted_start {
            return Err(NativeMappingAdmissionErrorV1::NonCumulativePrefix);
        }
        if cumulative_n_success > self.kfd_gpu_ids.len() {
            return Err(NativeMappingAdmissionErrorV1::ProgressOutOfRange);
        }
        self.mapped_prefix = cumulative_n_success;
        self.phase = match status {
            NativeProgressStatusV1::Succeeded if cumulative_n_success == self.kfd_gpu_ids.len() => {
                NativeMappingPhaseV1::Active
            }
            NativeProgressStatusV1::Succeeded => {
                return Err(NativeMappingAdmissionErrorV1::IncompleteSuccess);
            }
            NativeProgressStatusV1::Failed if self.mapped_prefix == 0 => {
                NativeMappingPhaseV1::Compensated
            }
            NativeProgressStatusV1::Failed => NativeMappingPhaseV1::Compensating,
            NativeProgressStatusV1::Indeterminate => NativeMappingPhaseV1::Quarantined,
        };
        if !self.validate_invariants() {
            return Err(NativeMappingAdmissionErrorV1::InvalidState);
        }
        Ok(self)
    }

    /// Begins deterministic teardown of a completely mapped device array.
    pub fn begin_unmap_model_only_v1(mut self) -> Result<Self, NativeMappingAdmissionErrorV1> {
        if !self.validate_invariants() {
            return Err(NativeMappingAdmissionErrorV1::InvalidState);
        }
        if self.phase != NativeMappingPhaseV1::Active {
            return Err(NativeMappingAdmissionErrorV1::InvalidPhase);
        }
        self.phase = NativeMappingPhaseV1::Compensating;
        Ok(self)
    }

    /// Applies the absolute cumulative `n_success` returned by one UNMAP result.
    pub fn observe_unmap_cumulative_prefix_model_only_v1(
        mut self,
        submitted_start: usize,
        cumulative_n_success: usize,
        status: NativeProgressStatusV1,
    ) -> Result<Self, NativeMappingAdmissionErrorV1> {
        if !self.validate_invariants() {
            return Err(NativeMappingAdmissionErrorV1::InvalidState);
        }
        if self.phase != NativeMappingPhaseV1::Compensating {
            return Err(NativeMappingAdmissionErrorV1::InvalidPhase);
        }
        if submitted_start != self.unmapped_prefix {
            return Err(NativeMappingAdmissionErrorV1::NonCumulativePrefix);
        }
        if cumulative_n_success < submitted_start {
            return Err(NativeMappingAdmissionErrorV1::NonCumulativePrefix);
        }
        if cumulative_n_success > self.mapped_prefix {
            return Err(NativeMappingAdmissionErrorV1::ProgressOutOfRange);
        }
        self.unmapped_prefix = cumulative_n_success;
        self.phase = match status {
            NativeProgressStatusV1::Succeeded if cumulative_n_success == self.mapped_prefix => {
                NativeMappingPhaseV1::Compensated
            }
            NativeProgressStatusV1::Succeeded => {
                return Err(NativeMappingAdmissionErrorV1::IncompleteSuccess);
            }
            NativeProgressStatusV1::Failed => {
                if self.unmapped_prefix == self.mapped_prefix {
                    NativeMappingPhaseV1::Quarantined
                } else {
                    NativeMappingPhaseV1::Compensating
                }
            }
            NativeProgressStatusV1::Indeterminate => NativeMappingPhaseV1::Quarantined,
        };
        if !self.validate_invariants() {
            return Err(NativeMappingAdmissionErrorV1::InvalidState);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeMappingAdmissionErrorV1 {
    InvalidSchema,
    InvalidIdentity,
    EmptyDeviceSet,
    DeviceCapacityExceeded,
    InvalidGpuId,
    NonCanonicalDeviceSet,
    InvalidState,
    InvalidPhase,
    NonCumulativePrefix,
    ProgressOutOfRange,
    IncompleteSuccess,
}

pub fn begin_native_multi_device_mapping_model_only_v1(
    observation: UntrustedNativeMultiDeviceMappingV1,
) -> Result<ModelNativeMultiDeviceMappingV1, NativeMappingAdmissionErrorV1> {
    if observation.schema_version != R9_NATIVE_EVIDENCE_SCHEMA_VERSION_V1 {
        return Err(NativeMappingAdmissionErrorV1::InvalidSchema);
    }
    if identity_digest_is_zero(observation.operation_identity)
        || identity_digest_is_zero(observation.allocation_identity)
    {
        return Err(NativeMappingAdmissionErrorV1::InvalidIdentity);
    }
    if observation.kfd_gpu_ids.is_empty() {
        return Err(NativeMappingAdmissionErrorV1::EmptyDeviceSet);
    }
    if observation.kfd_gpu_ids.len() > MAX_NATIVE_MAPPING_DEVICES_V1 {
        return Err(NativeMappingAdmissionErrorV1::DeviceCapacityExceeded);
    }
    if observation.kfd_gpu_ids.contains(&0) {
        return Err(NativeMappingAdmissionErrorV1::InvalidGpuId);
    }
    if observation
        .kfd_gpu_ids
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(NativeMappingAdmissionErrorV1::NonCanonicalDeviceSet);
    }
    Ok(ModelNativeMultiDeviceMappingV1 {
        operation_identity: observation.operation_identity,
        allocation_identity: observation.allocation_identity,
        kfd_gpu_ids: observation.kfd_gpu_ids,
        mapped_prefix: 0,
        unmapped_prefix: 0,
        phase: NativeMappingPhaseV1::Mapping,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedNativeXgmiRouteObservationV1 {
    pub schema_version: u16,
    pub route_identity: IdentityDigestV1,
    pub topology_identity: IdentityDigestV1,
    pub topology_generation: u64,
    pub observation_epoch: ObservationEpochV1,
    pub source_device: DeviceKeyV1,
    pub destination_device: DeviceKeyV1,
    pub source_kfd_gpu_id: u32,
    pub destination_kfd_gpu_id: u32,
    pub source_node_id: u32,
    pub destination_node_id: u32,
    pub hive_id: u64,
    pub io_link_index: u32,
    pub link_type: u32,
    pub min_bandwidth: u64,
    pub max_bandwidth: u64,
    pub recommended_transfer_size: u64,
    pub recommended_sdma_engine_id_mask: u64,
    pub selected_sdma_engine_id: u32,
    pub link_flags: u32,
    pub peer_access_supported: bool,
    pub sdma_xgmi_queue_supported: bool,
}

/// Separately sampled currentness coordinates for a route observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedNativeXgmiCurrentnessV1 {
    pub route_identity: IdentityDigestV1,
    pub topology_identity: IdentityDigestV1,
    pub topology_generation: u64,
    pub observation_epoch: ObservationEpochV1,
    pub source_device: DeviceKeyV1,
    pub destination_device: DeviceKeyV1,
    pub source_kfd_gpu_id: u32,
    pub destination_kfd_gpu_id: u32,
    pub source_node_id: u32,
    pub destination_node_id: u32,
    pub hive_id: u64,
    pub io_link_index: u32,
    pub link_type: u32,
    pub min_bandwidth: u64,
    pub max_bandwidth: u64,
    pub recommended_transfer_size: u64,
    pub recommended_sdma_engine_id_mask: u64,
    pub selected_sdma_engine_id: u32,
    pub link_flags: u32,
    pub reset_fence_current: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelNativeXgmiRouteV1 {
    observation: UntrustedNativeXgmiRouteObservationV1,
}

impl ModelNativeXgmiRouteV1 {
    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn observation(self) -> UntrustedNativeXgmiRouteObservationV1 {
        self.observation
    }

    pub const fn source_device(self) -> DeviceKeyV1 {
        self.observation.source_device
    }

    pub const fn destination_device(self) -> DeviceKeyV1 {
        self.observation.destination_device
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeXgmiRouteAdmissionErrorV1 {
    InvalidSchema,
    InvalidIdentity,
    SameDevice,
    SameGpuId,
    InvalidDirectionalLink,
    InvalidBandwidth,
    InvalidEngineSelection,
    PeerAccessUnavailable,
    XgmiQueueUnavailable,
    MappingNotActive,
    MappingDeviceSetMismatch,
    CurrentnessMismatch,
    ResetFenceNotCurrent,
}

pub fn admit_native_xgmi_route_model_only_v1(
    mapping: &ModelNativeMultiDeviceMappingV1,
    observation: UntrustedNativeXgmiRouteObservationV1,
    currentness: UntrustedNativeXgmiCurrentnessV1,
) -> Result<ModelNativeXgmiRouteV1, NativeXgmiRouteAdmissionErrorV1> {
    if observation.schema_version != R9_NATIVE_EVIDENCE_SCHEMA_VERSION_V1 {
        return Err(NativeXgmiRouteAdmissionErrorV1::InvalidSchema);
    }
    if identity_digest_is_zero(observation.route_identity)
        || identity_digest_is_zero(observation.topology_identity)
        || observation.topology_generation == 0
        || observation.observation_epoch.0 == 0
        || observation.source_device.generation.0 == 0
        || observation.destination_device.generation.0 == 0
        || observation.hive_id == 0
    {
        return Err(NativeXgmiRouteAdmissionErrorV1::InvalidIdentity);
    }
    if observation.source_device.physical == observation.destination_device.physical {
        return Err(NativeXgmiRouteAdmissionErrorV1::SameDevice);
    }
    if observation.source_kfd_gpu_id == 0
        || observation.destination_kfd_gpu_id == 0
        || observation.source_kfd_gpu_id == observation.destination_kfd_gpu_id
    {
        return Err(NativeXgmiRouteAdmissionErrorV1::SameGpuId);
    }
    if !observation.peer_access_supported {
        return Err(NativeXgmiRouteAdmissionErrorV1::PeerAccessUnavailable);
    }
    if !observation.sdma_xgmi_queue_supported {
        return Err(NativeXgmiRouteAdmissionErrorV1::XgmiQueueUnavailable);
    }
    if observation.source_node_id == observation.destination_node_id
        || observation.link_type != KFD_XGMI_LINK_TYPE_V1
        || observation.link_flags & KFD_XGMI_LINK_ENABLED_FLAG_V1 == 0
    {
        return Err(NativeXgmiRouteAdmissionErrorV1::InvalidDirectionalLink);
    }
    if observation.max_bandwidth == 0 || observation.min_bandwidth > observation.max_bandwidth {
        return Err(NativeXgmiRouteAdmissionErrorV1::InvalidBandwidth);
    }
    if observation.recommended_sdma_engine_id_mask.count_ones() != 1
        || observation.recommended_sdma_engine_id_mask.trailing_zeros()
            != observation.selected_sdma_engine_id
        || !(GFX942_FIRST_XGMI_SDMA_ENGINE_ID_V1..GFX942_SDMA_ENGINE_ID_LIMIT_V1)
            .contains(&observation.selected_sdma_engine_id)
    {
        return Err(NativeXgmiRouteAdmissionErrorV1::InvalidEngineSelection);
    }
    if mapping.phase != NativeMappingPhaseV1::Active || !mapping.validate_invariants() {
        return Err(NativeXgmiRouteAdmissionErrorV1::MappingNotActive);
    }
    let (first, second) = if observation.source_kfd_gpu_id < observation.destination_kfd_gpu_id {
        (
            observation.source_kfd_gpu_id,
            observation.destination_kfd_gpu_id,
        )
    } else {
        (
            observation.destination_kfd_gpu_id,
            observation.source_kfd_gpu_id,
        )
    };
    if mapping.kfd_gpu_ids.as_slice() != [first, second] {
        return Err(NativeXgmiRouteAdmissionErrorV1::MappingDeviceSetMismatch);
    }
    if currentness.route_identity != observation.route_identity
        || currentness.topology_identity != observation.topology_identity
        || currentness.topology_generation != observation.topology_generation
        || currentness.observation_epoch != observation.observation_epoch
        || currentness.source_device != observation.source_device
        || currentness.destination_device != observation.destination_device
        || currentness.source_kfd_gpu_id != observation.source_kfd_gpu_id
        || currentness.destination_kfd_gpu_id != observation.destination_kfd_gpu_id
        || currentness.source_node_id != observation.source_node_id
        || currentness.destination_node_id != observation.destination_node_id
        || currentness.hive_id != observation.hive_id
        || currentness.io_link_index != observation.io_link_index
        || currentness.link_type != observation.link_type
        || currentness.min_bandwidth != observation.min_bandwidth
        || currentness.max_bandwidth != observation.max_bandwidth
        || currentness.recommended_transfer_size != observation.recommended_transfer_size
        || currentness.recommended_sdma_engine_id_mask
            != observation.recommended_sdma_engine_id_mask
        || currentness.selected_sdma_engine_id != observation.selected_sdma_engine_id
        || currentness.link_flags != observation.link_flags
    {
        return Err(NativeXgmiRouteAdmissionErrorV1::CurrentnessMismatch);
    }
    if !currentness.reset_fence_current {
        return Err(NativeXgmiRouteAdmissionErrorV1::ResetFenceNotCurrent);
    }
    Ok(ModelNativeXgmiRouteV1 { observation })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942Cov6MachineTargetV1 {
    pub elf_machine: u16,
    pub code_object_version: u8,
    pub gfx_architecture: u16,
    pub xnack_disabled: bool,
    pub wavefront_size: u8,
}

impl Gfx942Cov6MachineTargetV1 {
    pub const fn exact_v1() -> Self {
        Self {
            elf_machine: AMDGPU_ELF_MACHINE_V1,
            code_object_version: GFX942_COV6_ABI_VERSION_V1,
            gfx_architecture: 942,
            xnack_disabled: true,
            wavefront_size: GFX942_COV6_WAVEFRONT_SIZE_V1,
        }
    }

    pub const fn is_exact_v1(self) -> bool {
        self.elf_machine == AMDGPU_ELF_MACHINE_V1
            && self.code_object_version == GFX942_COV6_ABI_VERSION_V1
            && self.gfx_architecture == 942
            && self.xnack_disabled
            && self.wavefront_size == GFX942_COV6_WAVEFRONT_SIZE_V1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedMachineCodeEvidenceAttestationV1 {
    pub schema_version: u16,
    pub attestation_identity: IdentityDigestV1,
    pub artifact: RuntimeArtifactIdV1,
    pub target: Gfx942Cov6MachineTargetV1,
    pub kernel_symbol_identity: IdentityDigestV1,
    pub kernel_descriptor_digest: IdentityDigestV1,
    pub machine_code_digest: IdentityDigestV1,
    /// Identity of a checked decoder/classifier receipt, not a proof that the
    /// decoded instructions implement the declared high-level semantics.
    pub checked_instruction_class_receipt_digest: IdentityDigestV1,
    pub semantic_contract_identity: IdentityDigestV1,
    pub kernel_identity: IdentityDigestV1,
    pub toolchain_identity: IdentityDigestV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedLoadedMachineCodeObservationV1 {
    pub loaded_code: LoadedCodeKeyV1,
    pub device: DeviceKeyV1,
    pub artifact: RuntimeArtifactIdV1,
    pub target: Gfx942Cov6MachineTargetV1,
    pub kernel_symbol_identity: IdentityDigestV1,
    pub kernel_descriptor_digest: IdentityDigestV1,
    pub machine_code_digest: IdentityDigestV1,
    /// Exact checked receipt associated with these loaded bytes.
    pub checked_instruction_class_receipt_digest: IdentityDigestV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelMachineCodeEvidenceBindingV1 {
    attestation: UntrustedMachineCodeEvidenceAttestationV1,
    loaded: UntrustedLoadedMachineCodeObservationV1,
}

impl ModelMachineCodeEvidenceBindingV1 {
    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn attestation(self) -> UntrustedMachineCodeEvidenceAttestationV1 {
        self.attestation
    }

    pub const fn loaded(self) -> UntrustedLoadedMachineCodeObservationV1 {
        self.loaded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineCodeEvidenceBindingErrorV1 {
    InvalidSchema,
    InvalidIdentity,
    UnsupportedTarget,
    SemanticContractMismatch,
    KernelIdentityMismatch,
    LoadedCodeMismatch,
    DeviceMismatch,
    ArtifactMismatch,
    TargetMismatch,
    SymbolMismatch,
    DescriptorMismatch,
    MachineCodeMismatch,
    InstructionClassReceiptMismatch,
}

pub fn bind_machine_code_evidence_model_only_v1(
    semantics: &ModelGfx942KernelSemanticsV1,
    attestation: UntrustedMachineCodeEvidenceAttestationV1,
    loaded: UntrustedLoadedMachineCodeObservationV1,
) -> Result<ModelMachineCodeEvidenceBindingV1, MachineCodeEvidenceBindingErrorV1> {
    if attestation.schema_version != R9_NATIVE_EVIDENCE_SCHEMA_VERSION_V1 {
        return Err(MachineCodeEvidenceBindingErrorV1::InvalidSchema);
    }
    if [
        attestation.attestation_identity,
        attestation.kernel_symbol_identity,
        attestation.kernel_descriptor_digest,
        attestation.machine_code_digest,
        attestation.checked_instruction_class_receipt_digest,
        attestation.semantic_contract_identity,
        attestation.kernel_identity,
        attestation.toolchain_identity,
    ]
    .into_iter()
    .any(identity_digest_is_zero)
        || identity_digest_is_zero(attestation.artifact.digest())
    {
        return Err(MachineCodeEvidenceBindingErrorV1::InvalidIdentity);
    }
    if !attestation.target.is_exact_v1() {
        return Err(MachineCodeEvidenceBindingErrorV1::UnsupportedTarget);
    }
    if attestation.semantic_contract_identity != semantics.contract_identity() {
        return Err(MachineCodeEvidenceBindingErrorV1::SemanticContractMismatch);
    }
    if attestation.kernel_identity != semantics.kernel_identity() {
        return Err(MachineCodeEvidenceBindingErrorV1::KernelIdentityMismatch);
    }
    if loaded.loaded_code != semantics.code() {
        return Err(MachineCodeEvidenceBindingErrorV1::LoadedCodeMismatch);
    }
    if loaded.device != semantics.device() {
        return Err(MachineCodeEvidenceBindingErrorV1::DeviceMismatch);
    }
    if attestation.artifact != semantics.artifact() || loaded.artifact != attestation.artifact {
        return Err(MachineCodeEvidenceBindingErrorV1::ArtifactMismatch);
    }
    if loaded.target != attestation.target {
        return Err(MachineCodeEvidenceBindingErrorV1::TargetMismatch);
    }
    if loaded.kernel_symbol_identity != attestation.kernel_symbol_identity {
        return Err(MachineCodeEvidenceBindingErrorV1::SymbolMismatch);
    }
    if loaded.kernel_descriptor_digest != attestation.kernel_descriptor_digest {
        return Err(MachineCodeEvidenceBindingErrorV1::DescriptorMismatch);
    }
    if loaded.machine_code_digest != attestation.machine_code_digest {
        return Err(MachineCodeEvidenceBindingErrorV1::MachineCodeMismatch);
    }
    if loaded.checked_instruction_class_receipt_digest
        != attestation.checked_instruction_class_receipt_digest
    {
        return Err(MachineCodeEvidenceBindingErrorV1::InstructionClassReceiptMismatch);
    }
    Ok(ModelMachineCodeEvidenceBindingV1 {
        attestation,
        loaded,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedMachineCodeDispatchCurrentnessV1 {
    pub dispatch_identity: IdentityDigestV1,
    pub loaded_code: LoadedCodeKeyV1,
    pub device: DeviceKeyV1,
    pub artifact: RuntimeArtifactIdV1,
    pub target: Gfx942Cov6MachineTargetV1,
    pub attestation_identity: IdentityDigestV1,
    pub kernel_symbol_identity: IdentityDigestV1,
    pub kernel_descriptor_digest: IdentityDigestV1,
    pub machine_code_digest: IdentityDigestV1,
    pub checked_instruction_class_receipt_digest: IdentityDigestV1,
    pub semantic_contract_identity: IdentityDigestV1,
    pub kernel_identity: IdentityDigestV1,
    pub toolchain_identity: IdentityDigestV1,
    pub device_current: bool,
    pub code_current: bool,
    pub mappings_current: bool,
    pub queue_current: bool,
    pub reset_fence_current: bool,
    pub dependency_frontier: u64,
    pub completed_frontier: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelMachineCodeDispatchV1 {
    dispatch_identity: IdentityDigestV1,
    binding: ModelMachineCodeEvidenceBindingV1,
    dependency_frontier: u64,
}

impl ModelMachineCodeDispatchV1 {
    pub const fn authority_domain(self) -> AuthorityDomainV1 {
        AuthorityDomainV1::ModelOnly
    }

    pub const fn dispatch_identity(self) -> IdentityDigestV1 {
        self.dispatch_identity
    }

    pub const fn binding(self) -> ModelMachineCodeEvidenceBindingV1 {
        self.binding
    }

    pub const fn dependency_frontier(self) -> u64 {
        self.dependency_frontier
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineCodeDispatchAdmissionErrorV1 {
    InvalidIdentity,
    BindingMismatch,
    EvidenceNotCurrent,
    DependencyIncomplete,
}

/// Admits publication only after all exact model evidence is revalidated.
pub fn admit_machine_code_dispatch_model_only_v1(
    binding: ModelMachineCodeEvidenceBindingV1,
    currentness: UntrustedMachineCodeDispatchCurrentnessV1,
) -> Result<ModelMachineCodeDispatchV1, MachineCodeDispatchAdmissionErrorV1> {
    if identity_digest_is_zero(currentness.dispatch_identity) {
        return Err(MachineCodeDispatchAdmissionErrorV1::InvalidIdentity);
    }
    let attestation = binding.attestation;
    let loaded = binding.loaded;
    if currentness.loaded_code != loaded.loaded_code
        || currentness.device != loaded.device
        || currentness.artifact != loaded.artifact
        || currentness.target != loaded.target
        || currentness.attestation_identity != attestation.attestation_identity
        || currentness.kernel_symbol_identity != loaded.kernel_symbol_identity
        || currentness.kernel_descriptor_digest != loaded.kernel_descriptor_digest
        || currentness.machine_code_digest != loaded.machine_code_digest
        || currentness.checked_instruction_class_receipt_digest
            != loaded.checked_instruction_class_receipt_digest
        || currentness.semantic_contract_identity != attestation.semantic_contract_identity
        || currentness.kernel_identity != attestation.kernel_identity
        || currentness.toolchain_identity != attestation.toolchain_identity
    {
        return Err(MachineCodeDispatchAdmissionErrorV1::BindingMismatch);
    }
    if !currentness.device_current
        || !currentness.code_current
        || !currentness.mappings_current
        || !currentness.queue_current
        || !currentness.reset_fence_current
    {
        return Err(MachineCodeDispatchAdmissionErrorV1::EvidenceNotCurrent);
    }
    if currentness.completed_frontier < currentness.dependency_frontier {
        return Err(MachineCodeDispatchAdmissionErrorV1::DependencyIncomplete);
    }
    Ok(ModelMachineCodeDispatchV1 {
        dispatch_identity: currentness.dispatch_identity,
        binding,
        dependency_frontier: currentness.dependency_frontier,
    })
}

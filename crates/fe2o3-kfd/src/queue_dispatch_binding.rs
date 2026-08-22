//! Private dispatch binding for the retained gfx942 compute-AQL queue.
//!
//! This module closes host-side identity, layout, ownership, and lifetime
//! composition. It deliberately has no public constructor or submission
//! method: device-data initialization/effect premises and native execution
//! semantics remain reviewed integration obligations.

#![allow(dead_code)]

use core::fmt;

use fe2o3_amdhsa_loader::{KernelIdentityInputsV1, ValidatedKernelEnvelope};
use fe2o3_aql::{
    AQL_MAX_FIXED_BATCH_PACKETS_V2, AqlDispatchGeometryV1, AqlRingCapacityV1, ObservedGpuAddressV1,
};
use fe2o3_hsaco::{ArgumentAccess, ArgumentAddressSpace, ExplicitValueKind};
use fe2o3_runtime_model::{MemoryMappingKeyV1, QueueKeyV1};
use sha2::{Digest, Sha256};

use super::completion::{
    CompletionDispatchGenerationBindingV1, CompletionPacketTemplateV1, Gfx942CompletedBatchV1,
    Gfx942CompletionBatchV1, Gfx942CompletionErrorV1, Gfx942CompletionPollV1,
};
use super::device_content::Gfx942DeviceContentDescriptorV1;
use crate::MemorySessionError;
use crate::shared_memory::{
    AqlDispatchCodeResourceRoleV1, AqlDispatchKernargResourceRoleV1, ExecutableGttV1,
    Gfx942DeviceMemoryDispatchAuthorityV1, Gfx942DeviceMemoryLayoutV1, Gfx942DeviceMemoryLeaseV1,
    Gfx942DeviceMemoryMappedV1, Gfx942InitializedDeviceMemoryV1, GttGpuAccessibleExecutableV1,
    GttGpuAccessibleMutableV1, KernargGttV1, SharedGttMemorySessionV1,
    SharedGttQueueResourceAuthorityV1,
};

pub(crate) const MAX_DISPATCH_DATA_LEASES_V1: usize = 16;
pub(crate) const MAX_DISPATCH_KERNARG_BYTES_V1: usize = 65_536;
pub const GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1: usize = 32;
pub const GFX942_MAX_FIXED_DISPATCH_PACKETS_V1: usize = AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize;
const KERNEL_DESCRIPTOR_BYTES_V1: u64 = 64;

/// One inert device-buffer field in a fixed dispatch kernarg image.
///
/// This value identifies an inspected explicit-argument ordinal and a bounded
/// subrange of a separately owned device allocation. It contains no native
/// address and grants no initialization, access-effect, or dispatch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942DispatchBufferBindingV1 {
    explicit_argument_index: usize,
    data_index: usize,
    data_byte_offset: u64,
    byte_len: u64,
}

impl Gfx942DispatchBufferBindingV1 {
    pub const fn new(
        explicit_argument_index: usize,
        data_index: usize,
        data_byte_offset: u64,
        byte_len: u64,
    ) -> Self {
        Self {
            explicit_argument_index,
            data_index,
            data_byte_offset,
            byte_len,
        }
    }
}

/// One inert packet description for a checked fixed dispatch batch.
///
/// The caller supplies scalar bytes, geometry, and buffer indices. Every
/// device-pointer field must be zero. Queue construction derives pointer
/// locations, alignments, and access effects from the selected inspected
/// kernel, validates all subranges, and performs numeric address substitution
/// only inside the retained native owner.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942FixedDispatchPacketV1;
///
/// fn cannot_extract_kernarg(packet: Gfx942FixedDispatchPacketV1) {
///     let _ = packet.kernarg_bytes;
/// }
/// ```
pub struct Gfx942FixedDispatchPacketV1 {
    program_index: usize,
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
    kernarg_bytes: Box<[u8]>,
    buffers: Box<[Gfx942DispatchBufferBindingV1]>,
}

impl Gfx942FixedDispatchPacketV1 {
    pub fn new(
        program_index: usize,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        kernarg_bytes: Box<[u8]>,
        buffers: Box<[Gfx942DispatchBufferBindingV1]>,
    ) -> Self {
        Self {
            program_index,
            geometry,
            dynamic_group_segment_bytes,
            kernarg_bytes,
            buffers,
        }
    }

    pub const fn program_index(&self) -> usize {
        self.program_index
    }

    pub const fn geometry(&self) -> AqlDispatchGeometryV1 {
        self.geometry
    }

    pub const fn dynamic_group_segment_bytes(&self) -> u32 {
        self.dynamic_group_segment_bytes
    }

    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }
}

impl fmt::Debug for Gfx942FixedDispatchPacketV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942FixedDispatchPacketV1")
            .field("program_index", &self.program_index)
            .field("geometry", &self.geometry)
            .field(
                "dynamic_group_segment_bytes",
                &self.dynamic_group_segment_bytes,
            )
            .field("kernarg_bytes", &self.kernarg_bytes.len())
            .field("buffer_count", &self.buffers.len())
            .finish_non_exhaustive()
    }
}

enum DispatchDataStorageV1 {
    Uninitialized(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
    InitializedContent(Gfx942InitializedDeviceMemoryV1),
    InitializedAfterDispatch(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
}

/// Move-only device-local allocation input for fixed dispatch composition.
///
/// Uninitialized storage is admitted only for inspected write-only arguments.
/// Read-only and read-write arguments require the sealed initialized variant.
/// Neither variant exposes a native address, allocation handle, or generation.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942FixedDispatchDataV1;
///
/// fn cannot_clone(data: Gfx942FixedDispatchDataV1) {
///     let _ = data.clone();
/// }
/// ```
pub struct Gfx942FixedDispatchDataV1 {
    storage: DispatchDataStorageV1,
}

impl Gfx942FixedDispatchDataV1 {
    pub fn uninitialized(lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>) -> Self {
        Self {
            storage: DispatchDataStorageV1::Uninitialized(lease),
        }
    }

    pub fn initialized(memory: Gfx942InitializedDeviceMemoryV1) -> Self {
        Self {
            storage: DispatchDataStorageV1::InitializedContent(memory),
        }
    }

    pub(super) fn initialized_after_dispatch(
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Self {
        Self {
            storage: DispatchDataStorageV1::InitializedAfterDispatch(lease),
        }
    }

    pub const fn layout(&self) -> Gfx942DeviceMemoryLayoutV1 {
        match &self.storage {
            DispatchDataStorageV1::Uninitialized(lease) => lease.layout(),
            DispatchDataStorageV1::InitializedContent(memory) => memory.layout(),
            DispatchDataStorageV1::InitializedAfterDispatch(lease) => lease.layout(),
        }
    }

    /// Returns whether the complete requested extent has initialized bytes.
    ///
    /// This observation does not identify their current content after any
    /// device publication.
    pub const fn is_fully_initialized(&self) -> bool {
        !matches!(self.storage, DispatchDataStorageV1::Uninitialized(_))
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        Option<Gfx942DeviceContentDescriptorV1>,
        bool,
    ) {
        match self.storage {
            DispatchDataStorageV1::Uninitialized(lease) => (lease, None, false),
            DispatchDataStorageV1::InitializedContent(memory) => {
                let (lease, content) = memory.into_parts();
                (lease, Some(content), true)
            }
            DispatchDataStorageV1::InitializedAfterDispatch(lease) => (lease, None, true),
        }
    }
}

impl fmt::Debug for Gfx942FixedDispatchDataV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942FixedDispatchDataV1")
            .field("layout", &self.layout())
            .field("fully_initialized", &self.is_fully_initialized())
            .finish_non_exhaustive()
    }
}

/// Frozen claim boundary for the addressless fixed-dispatch binding slice.
pub const GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-aql-dispatch-binding-r4-v1\n",
    "target=gfx942:xnack-,COV6,one-selected-current-device-vm-and-queue-generation\n",
    "code=1-through-32-validated-amdhsa-kernel-envelopes,content-and-selected-descriptor-identity,exact-zero-then-copy-materialization-into-owned-gtt,read-only-seal-before-map,per-packet-program-selection,descriptor-resolution-with-checked-relative-arithmetic\n",
    "kernarg=public-inert-complete-byte-images,exact-inspected-size-and-power-of-two-alignment,no-hidden-or-implicit-runtime-fields,all-global-buffer-fields-zero,checked-nonoverlapping-8-byte-internal-device-pointer-patches,one-owned-kernarg-gtt-arena-with-N-distinct-checked-aligned-slices,initialized-before-map\n",
    "data=1-through-16-actual-linear-mapped-device-memory-leases,exact-device-vm-generation-and-complete-live-set,checked-bounded-subranges,inspected-actual-access-derived-internally,read-or-readwrite-requires-sealed-host-initialized-authority,write-only-admits-uninitialized-exclusive-lease\n",
    "batch=1-through-1024,aql-fixed-batch-v2,minimum-ring-packet-capacity-checked,all-program-code-owners,N-distinct-kernarg-slices,one-generation-bound-template-per-packet,one-reservation-one-write-counter-fetch-add-one-final-doorbell-and-one-signal-per-packet-composition\n",
    "retention=queue-owns-all-code-kernarg-and-device-leases-through-exact-ready-and-recycle,ordinary-destroy-releases-all,returning-destroy-requires-one-exact-recycled-generation-and-returns-actual-mapped-authorities-with-owning-memory-session,fully-initialized-state-preserved-without-stale-current-content-digest,initially-uninitialized-remains-uninitialized\n",
    "queue-transfer=ordinary-path-still-rejects-device-memory,dispatch-path-requires-exact-complete-distinct-set-of-every-live-mapped-c3-lease-before-model-mutation\n",
    "failure=all-layout-and-identity-validation-before-native-preparation;post-side-effect-failure,currentness,publication,completion,timeout,recycle-or-release-ambiguity-poisons-and-requires-teardown\n",
    "authority=public-linear-addressless-construction-submit-poll-wait-recycle-and-returning-destroy,no-address-handle-pointer-fd-packet-template-signal-or-mmio-export\n",
    "proof=bounded-host-state-machine-and-mock-fault-tests-only,no-concrete-verus-or-machine-refinement\n",
    "contracted=code-segment-permission-refinement,implicit-kernarg-producer,cpu-gpu-coherence,firmware-dispatch-effects-and-quiescence\n",
    "excluded=caller-effect-assertion,caller-initialization-assertion,public-packet-template,async-copy,device-address-export,peer-map,kernel-memory-effect-refinement,numerical-correctness,hardware-execution\n",
);

/// SHA-256 of [`GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1`].
pub const GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1: &str =
    "7dceb951b02a9368a6b558aec4a3aa8df9b0c56e4aecbd5f4ee8eff612edf76b";

type CodeAuthority = SharedGttQueueResourceAuthorityV1<
    AqlDispatchCodeResourceRoleV1,
    ExecutableGttV1,
    GttGpuAccessibleExecutableV1,
>;
type KernargAuthority = SharedGttQueueResourceAuthorityV1<
    AqlDispatchKernargResourceRoleV1,
    KernargGttV1,
    GttGpuAccessibleMutableV1,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeviceDataEffectV1 {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl DeviceDataEffectV1 {
    const fn reads(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite)
    }
}

/// Write-only premise for one whole uninitialized C3 allocation.
///
/// Read effects remain rejected until the device-content foundation can
/// consume an authenticated copy-kernel completion and the queue can return
/// its retained destination lease. There is intentionally no caller boolean.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeviceDataPremiseV1 {
    role_identity: [u8; 32],
    valid_bytes: u64,
    effect: DeviceDataEffectV1,
}

impl DeviceDataPremiseV1 {
    pub(crate) const fn new(
        role_identity: [u8; 32],
        valid_bytes: u64,
        effect: DeviceDataEffectV1,
    ) -> Self {
        Self {
            role_identity,
            valid_bytes,
            effect,
        }
    }
}

pub(crate) struct DeviceDataAllocationInputV1 {
    requested_bytes: u64,
    alignment: u64,
    premise: DeviceDataPremiseV1,
}

impl DeviceDataAllocationInputV1 {
    pub(crate) const fn new(
        requested_bytes: u64,
        alignment: u64,
        premise: DeviceDataPremiseV1,
    ) -> Self {
        Self {
            requested_bytes,
            alignment,
            premise,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DevicePointerPatchV1 {
    byte_offset: usize,
    data_index: usize,
    data_byte_offset: u64,
    required_bytes: u64,
    required_alignment: u64,
}

impl DevicePointerPatchV1 {
    pub(crate) const fn new(
        byte_offset: usize,
        data_index: usize,
        data_byte_offset: u64,
        required_bytes: u64,
        required_alignment: u64,
    ) -> Self {
        Self {
            byte_offset,
            data_index,
            data_byte_offset,
            required_bytes,
            required_alignment,
        }
    }
}

/// Complete typed kernarg image before private device-pointer substitution.
///
/// Bytes and patch locations have no public accessor or constructor.
pub(crate) struct TypedKernargImageV1 {
    layout_identity: [u8; 32],
    bytes: Box<[u8]>,
    device_pointers: Box<[DevicePointerPatchV1]>,
}

impl TypedKernargImageV1 {
    pub(crate) fn new(
        layout_identity: [u8; 32],
        bytes: Box<[u8]>,
        device_pointers: Box<[DevicePointerPatchV1]>,
    ) -> Self {
        Self {
            layout_identity,
            bytes,
            device_pointers,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DispatchGeometryV1 {
    geometry: AqlDispatchGeometryV1,
    dynamic_group_segment_bytes: u32,
}

impl DispatchGeometryV1 {
    pub(crate) const fn new(
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
    ) -> Self {
        Self {
            geometry,
            dynamic_group_segment_bytes,
        }
    }
}

#[derive(Debug)]
pub enum Gfx942DispatchBindingErrorV1 {
    ZeroPacketCount,
    PacketCountExceedsMaximum { requested: usize, maximum: usize },
    RingCapacity { requested: usize, capacity: u32 },
    ProgramCount { requested: usize, maximum: usize },
    DataLeaseCount { requested: usize, maximum: usize },
    InvalidCode(&'static str),
    InvalidKernarg { packet: usize, detail: &'static str },
    InvalidData { index: usize, detail: &'static str },
    Geometry { packet: usize, detail: &'static str },
    Memory(MemorySessionError),
    Completion(Gfx942CompletionErrorV1),
    WrongQueueGeneration,
    StaleDispatchGeneration,
    ResourcePhase,
    GenerationExhausted,
    Poisoned,
}

impl fmt::Display for Gfx942DispatchBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Gfx942DispatchBindingErrorV1 {}

impl From<MemorySessionError> for Gfx942DispatchBindingErrorV1 {
    fn from(value: MemorySessionError) -> Self {
        Self::Memory(value)
    }
}

impl From<Gfx942CompletionErrorV1> for Gfx942DispatchBindingErrorV1 {
    fn from(value: Gfx942CompletionErrorV1) -> Self {
        Self::Completion(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DispatchOwnerPhaseV1 {
    Prepared,
    InFlight { generation: u64 },
    Completed { generation: u64 },
    Poisoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DispatchGenerationOwnerV1 {
    next_generation: u64,
    phase: DispatchOwnerPhaseV1,
    recycled_generation: Option<u64>,
}

impl DispatchGenerationOwnerV1 {
    const fn new() -> Self {
        Self {
            next_generation: 1,
            phase: DispatchOwnerPhaseV1::Prepared,
            recycled_generation: None,
        }
    }

    fn next(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.ensure_prepared()?;
        let generation = self.next_generation;
        generation
            .checked_add(1)
            .ok_or(Gfx942DispatchBindingErrorV1::GenerationExhausted)?;
        Ok(generation)
    }

    fn commit_begin(&mut self, generation: u64) {
        debug_assert_eq!(self.next_generation, generation);
        self.next_generation = generation + 1;
        self.phase = DispatchOwnerPhaseV1::InFlight { generation };
        self.recycled_generation = None;
    }

    fn active(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        match self.phase {
            DispatchOwnerPhaseV1::InFlight { generation }
            | DispatchOwnerPhaseV1::Completed { generation } => Ok(generation),
            DispatchOwnerPhaseV1::Poisoned => Err(Gfx942DispatchBindingErrorV1::Poisoned),
            DispatchOwnerPhaseV1::Prepared => Err(Gfx942DispatchBindingErrorV1::ResourcePhase),
        }
    }

    fn cancel(&mut self, generation: u64) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require(DispatchOwnerPhaseV1::InFlight { generation })?;
        self.phase = DispatchOwnerPhaseV1::Prepared;
        Ok(())
    }

    fn complete(&mut self, generation: u64) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require(DispatchOwnerPhaseV1::InFlight { generation })?;
        self.phase = DispatchOwnerPhaseV1::Completed { generation };
        Ok(())
    }

    fn recycle(&mut self, generation: u64) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require(DispatchOwnerPhaseV1::Completed { generation })?;
        self.phase = DispatchOwnerPhaseV1::Prepared;
        self.recycled_generation = Some(generation);
        Ok(())
    }

    fn returned_generation(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.ensure_prepared()?;
        self.recycled_generation
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)
    }

    fn poison(&mut self) {
        self.phase = DispatchOwnerPhaseV1::Poisoned;
    }

    fn ensure_prepared(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        match self.phase {
            DispatchOwnerPhaseV1::Prepared => Ok(()),
            DispatchOwnerPhaseV1::Poisoned => Err(Gfx942DispatchBindingErrorV1::Poisoned),
            _ => Err(Gfx942DispatchBindingErrorV1::ResourcePhase),
        }
    }

    fn require(&self, expected: DispatchOwnerPhaseV1) -> Result<(), Gfx942DispatchBindingErrorV1> {
        match self.phase {
            DispatchOwnerPhaseV1::Poisoned => Err(Gfx942DispatchBindingErrorV1::Poisoned),
            actual if actual == expected => Ok(()),
            _ => Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolvedCodeIdentityV1 {
    authenticated: KernelIdentityInputsV1,
    materialized_sha256: [u8; 32],
    mapping: MemoryMappingKeyV1,
    descriptor_address: ObservedGpuAddressV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RetainedDataPremiseV1 {
    role_identity: [u8; 32],
    valid_bytes: u64,
    effect: DeviceDataEffectV1,
    initialized_content: Option<Gfx942DeviceContentDescriptorV1>,
    fully_initialized: bool,
}

/// One actual mapped C3 authority returned only after exact C4 recycle.
pub(super) struct ReturnedDispatchDataLeaseV1 {
    lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    premise: RetainedDataPremiseV1,
}

impl ReturnedDispatchDataLeaseV1 {
    pub(super) const fn role_identity(&self) -> [u8; 32] {
        self.premise.role_identity
    }

    pub(super) const fn valid_bytes(&self) -> u64 {
        self.premise.valid_bytes
    }

    pub(super) const fn effect(&self) -> DeviceDataEffectV1 {
        self.premise.effect
    }

    pub(super) const fn is_fully_initialized(&self) -> bool {
        self.premise.fully_initialized
    }

    pub(super) fn into_lease(self) -> Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1> {
        self.lease
    }
}

/// Exact returned C3 set from one completed and recycled dispatch generation.
pub(super) struct ReturnedDispatchDataV1 {
    generation: u64,
    data: Vec<ReturnedDispatchDataLeaseV1>,
}

impl ReturnedDispatchDataV1 {
    pub(super) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(super) fn data(&self) -> &[ReturnedDispatchDataLeaseV1] {
        &self.data
    }

    pub(super) fn into_data(self) -> Vec<ReturnedDispatchDataLeaseV1> {
        self.data
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PreparedDispatchPacketV1 {
    geometry: AqlDispatchGeometryV1,
    private_segment_size: u32,
    group_segment_size: u32,
    kernarg_address: ObservedGpuAddressV1,
    kernarg_alignment: u64,
    kernarg_mapping: MemoryMappingKeyV1,
    kernarg_layout_identity: [u8; 32],
    code_index: usize,
}

/// Queue-retained real resource owner for one prepared batch shape.
pub(super) struct DispatchResourceOwnerV1 {
    code: Vec<CodeAuthority>,
    code_identity: Vec<ResolvedCodeIdentityV1>,
    kernarg: KernargAuthority,
    packets: Vec<PreparedDispatchPacketV1>,
    data: Vec<Gfx942DeviceMemoryDispatchAuthorityV1>,
    data_premises: Vec<RetainedDataPremiseV1>,
    generation: DispatchGenerationOwnerV1,
}

impl DispatchResourceOwnerV1 {
    pub(super) fn device_authorities(&self) -> &[Gfx942DeviceMemoryDispatchAuthorityV1] {
        &self.data
    }

    pub(super) fn active_generation(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.generation.active()
    }

    pub(super) fn bind_templates<const N: usize>(
        &mut self,
        queue: QueueKeyV1,
    ) -> Result<[CompletionPacketTemplateV1; N], Gfx942DispatchBindingErrorV1> {
        self.require_prepared()?;
        validate_packet_count::<N>()?;
        if self.packets.len() != N
            || self.code_identity.len() != self.code.len()
            || self
                .code_identity
                .iter()
                .any(|identity| identity.mapping.allocation.vm != queue.vm)
            || self.kernarg.facts().mapping().allocation.vm != queue.vm
            || self
                .data
                .iter()
                .any(|authority| authority.facts().vm() != queue.vm)
        {
            return Err(Gfx942DispatchBindingErrorV1::WrongQueueGeneration);
        }
        let generation = self.generation.next()?;
        let templates: Vec<_> = self
            .packets
            .iter()
            .map(|packet| {
                let code = self.code_identity.get(packet.code_index).ok_or(
                    Gfx942DispatchBindingErrorV1::InvalidCode("packet program index"),
                )?;
                Ok(CompletionPacketTemplateV1::new(
                    packet.geometry,
                    packet.private_segment_size,
                    packet.group_segment_size,
                    code.descriptor_address,
                    packet.kernarg_address,
                    packet.kernarg_alignment,
                    CompletionDispatchGenerationBindingV1::new(
                        queue,
                        code.mapping,
                        packet.kernarg_mapping,
                        generation,
                    ),
                ))
            })
            .collect::<Result<Vec<_>, Gfx942DispatchBindingErrorV1>>()?;
        let templates =
            templates
                .try_into()
                .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidKernarg {
                    packet: 0,
                    detail: "prepared packet cardinality",
                })?;
        self.generation.commit_begin(generation);
        Ok(templates)
    }

    pub(super) fn cancel_binding(
        &mut self,
        generation: u64,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.cancel(generation)
    }

    pub(super) fn mark_completed(
        &mut self,
        generation: u64,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.complete(generation)
    }

    pub(super) fn mark_recycled(
        &mut self,
        generation: u64,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.recycle(generation)
    }

    pub(super) fn poison(&mut self) {
        self.generation.poison();
    }

    pub(super) fn ensure_releasable(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require_prepared()
    }

    pub(super) fn ensure_returnable(&self) -> Result<u64, Gfx942DispatchBindingErrorV1> {
        self.generation.returned_generation()
    }

    pub(super) fn release(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.require_prepared()?;
        let kernarg = memory.unmap_from_gpu(self.kernarg.into_token())?;
        memory.release(kernarg)?;
        for code in self.code {
            let code = memory.unmap_executable_from_gpu(code.into_token())?;
            memory.release_executable(code)?;
        }
        for data in self.data {
            let lease = memory.unmap_gfx942_device_memory(data.into_lease())?;
            memory.release_gfx942_device_memory(lease)?;
        }
        Ok(())
    }

    /// Releases code and kernarg while returning the exact mapped C3 set.
    ///
    /// The generation owner admits this transition only after a matching C4
    /// completion was observed and its signal was recycled. The returned
    /// authorities retain no public address, handle, pointer, or descriptor.
    pub(super) fn release_non_data_after_recycle(
        self,
        memory: &mut SharedGttMemorySessionV1,
    ) -> Result<ReturnedDispatchDataV1, Gfx942DispatchBindingErrorV1> {
        let generation = self.generation.returned_generation()?;
        if self.data.len() != self.data_premises.len() {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: self.data.len().min(self.data_premises.len()),
                detail: "retained data/premise cardinality",
            });
        }
        let kernarg = memory.unmap_from_gpu(self.kernarg.into_token())?;
        memory.release(kernarg)?;
        for code in self.code {
            let code = memory.unmap_executable_from_gpu(code.into_token())?;
            memory.release_executable(code)?;
        }
        let data = self
            .data
            .into_iter()
            .zip(self.data_premises)
            .map(|(authority, premise)| ReturnedDispatchDataLeaseV1 {
                lease: authority.into_lease(),
                premise,
            })
            .collect();
        Ok(ReturnedDispatchDataV1 { generation, data })
    }

    fn require_prepared(&self) -> Result<(), Gfx942DispatchBindingErrorV1> {
        self.generation.ensure_prepared()
    }
}

/// Linear published dispatch batch retaining one exact resource generation.
///
/// It has no public constructor or operation, and is neither `Clone` nor
/// `Copy`.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942DispatchBatchV1;
///
/// fn consume<const N: usize>(_: Gfx942DispatchBatchV1<N>) {}
/// fn cannot_use_twice<const N: usize>(batch: Gfx942DispatchBatchV1<N>) {
///     consume(batch);
///     consume(batch);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942DispatchBatchV1;
///
/// fn cannot_clone<const N: usize>(batch: Gfx942DispatchBatchV1<N>) {
///     let _ = batch.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942DispatchBatchV1;
///
/// fn cannot_extract_addresses<const N: usize>(batch: &Gfx942DispatchBatchV1<N>) {
///     let _ = batch.kernel_object();
///     let _ = batch.kernarg_address();
///     let _ = batch.device_addresses();
/// }
/// ```
#[must_use = "a published dispatch batch must remain bound through completion"]
pub struct Gfx942DispatchBatchV1<const N: usize> {
    completion: Gfx942CompletionBatchV1<N>,
    generation: u64,
}

impl<const N: usize> fmt::Debug for Gfx942DispatchBatchV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942DispatchBatchV1")
            .field("packet_count", &N)
            .finish_non_exhaustive()
    }
}

/// Linear exact-batch completion before signal recycle.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942CompletedDispatchBatchV1;
///
/// fn consume<const N: usize>(_: Gfx942CompletedDispatchBatchV1<N>) {}
/// fn cannot_recycle_twice<const N: usize>(batch: Gfx942CompletedDispatchBatchV1<N>) {
///     consume(batch);
///     consume(batch);
/// }
/// ```
#[must_use = "completed dispatch resources remain retained until signal recycle"]
pub struct Gfx942CompletedDispatchBatchV1<const N: usize> {
    completion: Gfx942CompletedBatchV1<N>,
    generation: u64,
}

impl<const N: usize> fmt::Debug for Gfx942CompletedDispatchBatchV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942CompletedDispatchBatchV1")
            .field("packet_count", &N)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum Gfx942DispatchPollV1<const N: usize> {
    Pending(Gfx942DispatchBatchV1<N>),
    Ready(Gfx942CompletedDispatchBatchV1<N>),
}

pub(super) fn wrap_published<const N: usize>(
    completion: Gfx942CompletionBatchV1<N>,
    generation: u64,
) -> Gfx942DispatchBatchV1<N> {
    Gfx942DispatchBatchV1 {
        completion,
        generation,
    }
}

pub(super) fn unwrap_published<const N: usize>(
    batch: Gfx942DispatchBatchV1<N>,
) -> (Gfx942CompletionBatchV1<N>, u64) {
    (batch.completion, batch.generation)
}

pub(super) fn wrap_poll<const N: usize>(
    poll: Gfx942CompletionPollV1<N>,
    generation: u64,
) -> Gfx942DispatchPollV1<N> {
    match poll {
        Gfx942CompletionPollV1::Pending(completion) => {
            Gfx942DispatchPollV1::Pending(wrap_published(completion, generation))
        }
        Gfx942CompletionPollV1::Ready(completion) => {
            Gfx942DispatchPollV1::Ready(Gfx942CompletedDispatchBatchV1 {
                completion,
                generation,
            })
        }
    }
}

pub(super) fn unwrap_completed<const N: usize>(
    batch: Gfx942CompletedDispatchBatchV1<N>,
) -> (Gfx942CompletedBatchV1<N>, u64) {
    (batch.completion, batch.generation)
}

pub(super) fn wrap_completed<const N: usize>(
    completion: Gfx942CompletedBatchV1<N>,
    generation: u64,
) -> Gfx942CompletedDispatchBatchV1<N> {
    Gfx942CompletedDispatchBatchV1 {
        completion,
        generation,
    }
}

/// Builds real owned code, kernarg, and C3 data authorities without publishing.
pub(super) fn prepare_dispatch_resources<const N: usize>(
    memory: &mut SharedGttMemorySessionV1,
    kernel: ValidatedKernelEnvelope<'_>,
    geometry: [DispatchGeometryV1; N],
    kernargs: [TypedKernargImageV1; N],
    data: Vec<DeviceDataAllocationInputV1>,
) -> Result<DispatchResourceOwnerV1, Gfx942DispatchBindingErrorV1> {
    validate_packet_count::<N>()?;
    let resources = kernel.resources();
    let plan = *kernel.envelope().plan();
    let image_len_u64 = plan
        .image_end()
        .checked_sub(plan.image_start())
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode("image range"))?;
    let image_len = usize::try_from(image_len_u64)
        .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("image size conversion"))?;
    if image_len == 0 {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode("empty image"));
    }
    let descriptor_offset = kernel
        .selected_binding()
        .descriptor_address()
        .checked_sub(plan.image_start())
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
            "descriptor precedes image",
        ))?;
    descriptor_offset
        .checked_add(KERNEL_DESCRIPTOR_BYTES_V1)
        .filter(|end| *end <= image_len_u64)
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
            "descriptor outside image",
        ))?;
    if !descriptor_offset.is_multiple_of(KERNEL_DESCRIPTOR_BYTES_V1) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "descriptor alignment",
        ));
    }
    let kernarg_size = usize::try_from(resources.kernarg_segment_size())
        .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("kernarg size conversion"))?;
    if kernarg_size == 0 || kernarg_size > MAX_DISPATCH_KERNARG_BYTES_V1 {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "kernarg size bound",
        ));
    }
    let kernarg_alignment = resources.kernarg_segment_alignment();
    if kernarg_alignment == 0 || !kernarg_alignment.is_power_of_two() {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "kernarg alignment",
        ));
    }
    let kernarg_alignment = usize::try_from(kernarg_alignment)
        .ok()
        .filter(|alignment| *alignment <= 4096)
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
            "kernarg alignment bound",
        ))?;
    let kernarg_stride = kernarg_size
        .checked_add(kernarg_alignment - 1)
        .map(|bytes| bytes & !(kernarg_alignment - 1))
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode("kernarg stride"))?;
    let kernarg_arena_bytes =
        kernarg_stride
            .checked_mul(N)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
                "kernarg arena size",
            ))?;
    validate_geometry(resources, &geometry)?;
    validate_data_inputs(&data)?;
    validate_kernargs(&kernargs, kernarg_size, &data)?;
    let requests: Vec<_> = data
        .iter()
        .map(|input| (input.requested_bytes, input.alignment))
        .collect();
    memory.validate_gfx942_dispatch_allocation_requests(&requests)?;
    let mut data_authorities = Vec::with_capacity(data.len());
    let mut data_premises = Vec::with_capacity(data.len());
    for input in data {
        let premise = input.premise;
        let lease = memory.allocate_gfx942_device_memory(input.requested_bytes, input.alignment)?;
        let lease = memory.map_gfx942_device_memory(lease)?;
        let authority = memory.retain_gfx942_device_memory_for_dispatch(lease)?;
        data_premises.push(RetainedDataPremiseV1 {
            role_identity: premise.role_identity,
            valid_bytes: premise.valid_bytes,
            effect: premise.effect,
            initialized_content: None,
            fully_initialized: false,
        });
        data_authorities.push(authority);
    }

    let mut code = memory.allocate_executable(image_len)?;
    let materialized_sha256 = memory.with_bytes_mut(&mut code, |bytes| {
        kernel
            .materialize_into(bytes)
            .map(|()| Sha256::digest(bytes).into())
    })?;
    let materialized_sha256 = match materialized_sha256 {
        Ok(digest) => digest,
        Err(_) => {
            let _ = memory.quarantine_queue_composition("dispatch code materialization failure");
            return Err(Gfx942DispatchBindingErrorV1::InvalidCode("materialization"));
        }
    };
    let code = memory.seal_executable(code)?;
    let code = memory.map_executable_to_gpu(code)?;
    let code = memory.retain_aql_dispatch_code_resource(code)?;
    let descriptor_address = code
        .facts()
        .checked_gpu_subrange(descriptor_offset, KERNEL_DESCRIPTOR_BYTES_V1, 64)
        .and_then(|address| ObservedGpuAddressV1::new(address).ok())
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
            "resolved descriptor address",
        ))?;
    let code_identity = ResolvedCodeIdentityV1 {
        authenticated: kernel.identity_inputs(),
        materialized_sha256,
        mapping: code.facts().mapping(),
        descriptor_address,
    };

    let private_segment_size = u32::try_from(resources.private_segment_fixed_size())
        .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("private segment size"))?;
    let mut kernarg = memory.allocate_kernarg(kernarg_arena_bytes)?;
    memory.with_bytes_mut(&mut kernarg, |bytes| {
        bytes.fill(0);
        for (packet_index, typed) in kernargs.iter().enumerate() {
            let start = packet_index * kernarg_stride;
            let packet_bytes = &mut bytes[start..start + kernarg_size];
            packet_bytes.copy_from_slice(&typed.bytes);
            for patch in &typed.device_pointers {
                let address = data_authorities[patch.data_index]
                    .facts()
                    .checked_gpu_subrange(
                        patch.data_byte_offset,
                        patch.required_bytes,
                        patch.required_alignment,
                    )
                    .expect("dispatch preflight checked device pointer range");
                packet_bytes[patch.byte_offset..patch.byte_offset + 8]
                    .copy_from_slice(&address.to_le_bytes());
            }
        }
    })?;
    let kernarg = memory.map_to_gpu(kernarg)?;
    let kernarg = memory.retain_aql_dispatch_kernarg_resource(kernarg)?;
    let mut packets = Vec::with_capacity(N);
    for (packet_index, (typed, dispatch_geometry)) in kernargs.into_iter().zip(geometry).enumerate()
    {
        let kernarg_offset = packet_index
            .checked_mul(kernarg_stride)
            .and_then(|offset| u64::try_from(offset).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "kernarg slice offset",
            })?;
        let kernarg_address = kernarg
            .facts()
            .checked_gpu_subrange(
                kernarg_offset,
                kernarg_size as u64,
                kernarg_alignment as u64,
            )
            .and_then(|address| ObservedGpuAddressV1::new(address).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "mapped kernarg address",
            })?;
        let group_segment_size = u64::from(dispatch_geometry.dynamic_group_segment_bytes)
            .checked_add(resources.group_segment_fixed_size())
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::Geometry {
                packet: packet_index,
                detail: "group segment size",
            })?;
        packets.push(PreparedDispatchPacketV1 {
            geometry: dispatch_geometry.geometry,
            private_segment_size,
            group_segment_size,
            kernarg_address,
            kernarg_alignment: kernarg_alignment as u64,
            kernarg_mapping: kernarg.facts().mapping(),
            kernarg_layout_identity: typed.layout_identity,
            code_index: 0,
        });
    }

    Ok(DispatchResourceOwnerV1 {
        code: vec![code],
        code_identity: vec![code_identity],
        kernarg,
        packets,
        data: data_authorities,
        data_premises,
        generation: DispatchGenerationOwnerV1::new(),
    })
}

struct FixedDispatchProgramPlanV1 {
    image_len: usize,
    descriptor_offset: u64,
    resources: fe2o3_amdhsa_loader::SelectedKernelResourceBindingV1,
}

struct FixedDispatchPacketPlanV1 {
    input: Gfx942FixedDispatchPacketV1,
    patches: Box<[DevicePointerPatchV1]>,
    kernarg_offset: usize,
    kernarg_alignment: usize,
    private_segment_size: u32,
    group_segment_size: u32,
}

/// Consumes inspected executable custody and exact mapped data authorities,
/// then prepares one addressless fixed batch without publishing it.
pub(super) fn prepare_public_fixed_dispatch_resources<const N: usize>(
    memory: &mut SharedGttMemorySessionV1,
    programs: Vec<ValidatedKernelEnvelope<'_>>,
    packets: [Gfx942FixedDispatchPacketV1; N],
    data: Vec<Gfx942FixedDispatchDataV1>,
) -> Result<DispatchResourceOwnerV1, Gfx942DispatchBindingErrorV1> {
    validate_packet_count::<N>()?;
    if programs.is_empty() || programs.len() > GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1 {
        return Err(Gfx942DispatchBindingErrorV1::ProgramCount {
            requested: programs.len(),
            maximum: GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1,
        });
    }
    if data.is_empty() || data.len() > MAX_DISPATCH_DATA_LEASES_V1 {
        return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
            requested: data.len(),
            maximum: MAX_DISPATCH_DATA_LEASES_V1,
        });
    }
    let data_layouts: Vec<_> = data.iter().map(Gfx942FixedDispatchDataV1::layout).collect();
    let data_initialized: Vec<_> = data
        .iter()
        .map(Gfx942FixedDispatchDataV1::is_fully_initialized)
        .collect();
    let mut program_plans = Vec::with_capacity(programs.len());
    for kernel in &programs {
        let resources = kernel.resources();
        let plan = *kernel.envelope().plan();
        let image_len_u64 = plan
            .image_end()
            .checked_sub(plan.image_start())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode("image range"))?;
        let image_len = usize::try_from(image_len_u64)
            .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("image size conversion"))?;
        let descriptor_offset = kernel
            .selected_binding()
            .descriptor_address()
            .checked_sub(plan.image_start())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
                "descriptor precedes image",
            ))?;
        if image_len == 0
            || descriptor_offset
                .checked_add(KERNEL_DESCRIPTOR_BYTES_V1)
                .is_none_or(|end| end > image_len_u64)
            || !descriptor_offset.is_multiple_of(KERNEL_DESCRIPTOR_BYTES_V1)
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
                "descriptor image range",
            ));
        }
        let selected = kernel.selected_kernel();
        if !selected.hidden_arguments().is_empty()
            || selected.implicit_argument_offset().is_some()
            || selected.implicit_argument_size() != 0
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
                "runtime-populated kernarg fields",
            ));
        }
        validate_kernarg_resource_shape(resources)?;
        program_plans.push(FixedDispatchProgramPlanV1 {
            image_len,
            descriptor_offset,
            resources,
        });
    }

    let mut referenced_programs = vec![false; programs.len()];
    let mut referenced_data = vec![false; data.len()];
    let mut data_effects = vec![None; data.len()];
    let mut packet_plans = Vec::with_capacity(N);
    let mut kernarg_arena_bytes = 0usize;
    for (packet_index, input) in packets.into_iter().enumerate() {
        let program_plan = program_plans.get(input.program_index).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "program index",
            },
        )?;
        let kernel = &programs[input.program_index];
        referenced_programs[input.program_index] = true;
        let kernarg_size = usize::try_from(program_plan.resources.kernarg_segment_size())
            .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("kernarg size conversion"))?;
        if input.kernarg_bytes.len() != kernarg_size {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "exact kernarg byte extent",
            });
        }
        let kernarg_alignment = usize::try_from(program_plan.resources.kernarg_segment_alignment())
            .map_err(|_| {
                Gfx942DispatchBindingErrorV1::InvalidCode("kernarg alignment conversion")
            })?;
        let kernarg_offset = align_up(kernarg_arena_bytes, kernarg_alignment).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "kernarg arena offset",
            },
        )?;
        kernarg_arena_bytes = kernarg_offset.checked_add(kernarg_size).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: packet_index,
                detail: "kernarg arena extent",
            },
        )?;
        let patches = validate_public_packet_bindings(
            packet_index,
            kernel,
            &input,
            &data_layouts,
            &mut referenced_data,
            &mut data_effects,
        )?;
        let geometry = DispatchGeometryV1::new(input.geometry, input.dynamic_group_segment_bytes);
        validate_geometry(program_plan.resources, &[geometry])?;
        let private_segment_size =
            u32::try_from(program_plan.resources.private_segment_fixed_size())
                .map_err(|_| Gfx942DispatchBindingErrorV1::InvalidCode("private segment size"))?;
        let group_segment_size = u64::from(input.dynamic_group_segment_bytes)
            .checked_add(program_plan.resources.group_segment_fixed_size())
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::Geometry {
                packet: packet_index,
                detail: "group segment size",
            })?;
        packet_plans.push(FixedDispatchPacketPlanV1 {
            input,
            patches,
            kernarg_offset,
            kernarg_alignment,
            private_segment_size,
            group_segment_size,
        });
    }
    if let Some(index) = referenced_programs.iter().position(|value| !value) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(if index == 0 {
            "unreferenced first program"
        } else {
            "unreferenced program"
        }));
    }
    if let Some(index) = referenced_data.iter().position(|value| !value) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index,
            detail: "allocation not referenced by batch",
        });
    }
    validate_initialization_premises(&data_effects, &data_initialized)?;

    let mut data_authorities = Vec::with_capacity(data.len());
    let mut data_premises = Vec::with_capacity(data.len());
    for (index, input) in data.into_iter().enumerate() {
        let layout = input.layout();
        let (lease, initialized_content, fully_initialized) = input.into_parts();
        let authority = memory.retain_gfx942_device_memory_for_dispatch(lease)?;
        data_authorities.push(authority);
        data_premises.push(RetainedDataPremiseV1 {
            role_identity: [0; 32],
            valid_bytes: layout.requested_bytes(),
            effect: data_effects[index].expect("referenced data has an inspected effect"),
            initialized_content,
            fully_initialized,
        });
    }

    let mut code = Vec::with_capacity(programs.len());
    let mut code_identity = Vec::with_capacity(programs.len());
    for (kernel, plan) in programs.into_iter().zip(&program_plans) {
        let mut allocation = memory.allocate_executable(plan.image_len)?;
        let materialized_sha256 = memory.with_bytes_mut(&mut allocation, |bytes| {
            kernel
                .materialize_into(bytes)
                .map(|()| Sha256::digest(bytes).into())
        })?;
        let materialized_sha256 = match materialized_sha256 {
            Ok(digest) => digest,
            Err(_) => {
                let _ =
                    memory.quarantine_queue_composition("dispatch code materialization failure");
                return Err(Gfx942DispatchBindingErrorV1::InvalidCode("materialization"));
            }
        };
        let allocation = memory.seal_executable(allocation)?;
        let allocation = memory.map_executable_to_gpu(allocation)?;
        let allocation = memory.retain_aql_dispatch_code_resource(allocation)?;
        let descriptor_address = allocation
            .facts()
            .checked_gpu_subrange(plan.descriptor_offset, KERNEL_DESCRIPTOR_BYTES_V1, 64)
            .and_then(|address| ObservedGpuAddressV1::new(address).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidCode(
                "resolved descriptor address",
            ))?;
        code_identity.push(ResolvedCodeIdentityV1 {
            authenticated: kernel.identity_inputs(),
            materialized_sha256,
            mapping: allocation.facts().mapping(),
            descriptor_address,
        });
        code.push(allocation);
    }

    let mut kernarg = memory.allocate_kernarg(kernarg_arena_bytes)?;
    memory.with_bytes_mut(&mut kernarg, |bytes| {
        bytes.fill(0);
        for packet in &packet_plans {
            let start = packet.kernarg_offset;
            let end = start + packet.input.kernarg_bytes.len();
            let packet_bytes = &mut bytes[start..end];
            packet_bytes.copy_from_slice(&packet.input.kernarg_bytes);
            for patch in &packet.patches {
                let address = data_authorities[patch.data_index]
                    .facts()
                    .checked_gpu_subrange(
                        patch.data_byte_offset,
                        patch.required_bytes,
                        patch.required_alignment,
                    )
                    .expect("public dispatch preflight checked pointer range");
                packet_bytes[patch.byte_offset..patch.byte_offset + 8]
                    .copy_from_slice(&address.to_le_bytes());
            }
        }
    })?;
    let kernarg = memory.map_to_gpu(kernarg)?;
    let kernarg = memory.retain_aql_dispatch_kernarg_resource(kernarg)?;
    let mut prepared_packets = Vec::with_capacity(N);
    for packet in packet_plans {
        let kernarg_address = kernarg
            .facts()
            .checked_gpu_subrange(
                packet.kernarg_offset as u64,
                packet.input.kernarg_bytes.len() as u64,
                packet.kernarg_alignment as u64,
            )
            .and_then(|address| ObservedGpuAddressV1::new(address).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet: prepared_packets.len(),
                detail: "mapped kernarg address",
            })?;
        prepared_packets.push(PreparedDispatchPacketV1 {
            geometry: packet.input.geometry,
            private_segment_size: packet.private_segment_size,
            group_segment_size: packet.group_segment_size,
            kernarg_address,
            kernarg_alignment: packet.kernarg_alignment as u64,
            kernarg_mapping: kernarg.facts().mapping(),
            kernarg_layout_identity: code_identity[packet.input.program_index]
                .authenticated
                .closure_sha256(),
            code_index: packet.input.program_index,
        });
    }

    Ok(DispatchResourceOwnerV1 {
        code,
        code_identity,
        kernarg,
        packets: prepared_packets,
        data: data_authorities,
        data_premises,
        generation: DispatchGenerationOwnerV1::new(),
    })
}

fn validate_kernarg_resource_shape(
    resources: fe2o3_amdhsa_loader::SelectedKernelResourceBindingV1,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let size = resources.kernarg_segment_size();
    let alignment = resources.kernarg_segment_alignment();
    if size == 0
        || size > MAX_DISPATCH_KERNARG_BYTES_V1 as u64
        || alignment == 0
        || !alignment.is_power_of_two()
        || alignment > 4096
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidCode(
            "kernarg resource shape",
        ));
    }
    Ok(())
}

fn validate_public_packet_bindings(
    packet: usize,
    kernel: &ValidatedKernelEnvelope<'_>,
    input: &Gfx942FixedDispatchPacketV1,
    data: &[Gfx942DeviceMemoryLayoutV1],
    referenced_data: &mut [bool],
    data_effects: &mut [Option<DeviceDataEffectV1>],
) -> Result<Box<[DevicePointerPatchV1]>, Gfx942DispatchBindingErrorV1> {
    let arguments = kernel.selected_kernel().explicit_arguments();
    let global_count = arguments
        .iter()
        .filter(|argument| argument.value_kind() == ExplicitValueKind::GlobalBuffer)
        .count();
    if input.buffers.len() != global_count {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "global-buffer binding cardinality",
        });
    }
    let mut seen_arguments = vec![false; arguments.len()];
    let mut patches = Vec::with_capacity(input.buffers.len());
    for binding in &input.buffers {
        let argument = arguments.get(binding.explicit_argument_index).ok_or(
            Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "explicit argument index",
            },
        )?;
        if seen_arguments[binding.explicit_argument_index]
            || argument.value_kind() != ExplicitValueKind::GlobalBuffer
            || argument.size() != 8
            || argument.address_space() != Some(ArgumentAddressSpace::Global)
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "inspected global-buffer argument",
            });
        }
        seen_arguments[binding.explicit_argument_index] = true;
        let layout =
            data.get(binding.data_index)
                .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                    packet,
                    detail: "device data index",
                })?;
        let (patch, effect) = validate_inspected_buffer_contract(
            packet,
            &input.kernarg_bytes,
            binding,
            layout.requested_bytes(),
            layout.alignment(),
            &input.buffers,
            InspectedBufferContractV1 {
                pointer_offset: argument.offset(),
                declared_access: argument.access(),
                actual_access: argument.actual_access(),
                pointee_alignment: argument.pointee_alignment(),
            },
        )?;
        referenced_data[binding.data_index] = true;
        data_effects[binding.data_index] =
            Some(merge_effect(data_effects[binding.data_index], effect));
        if patches.iter().any(|prior: &DevicePointerPatchV1| {
            ranges_overlap_usize(prior.byte_offset, 8, patch.byte_offset, 8)
        }) {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "overlapping pointer fields",
            });
        }
        patches.push(patch);
    }
    if arguments.iter().enumerate().any(|(index, argument)| {
        argument.value_kind() == ExplicitValueKind::GlobalBuffer && !seen_arguments[index]
    }) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "missing inspected global-buffer binding",
        });
    }
    Ok(patches.into_boxed_slice())
}

#[derive(Clone, Copy)]
struct InspectedBufferContractV1 {
    pointer_offset: u64,
    declared_access: Option<ArgumentAccess>,
    actual_access: Option<ArgumentAccess>,
    pointee_alignment: Option<u64>,
}

fn validate_inspected_buffer_contract(
    packet: usize,
    kernarg_bytes: &[u8],
    binding: &Gfx942DispatchBufferBindingV1,
    allocation_bytes: u64,
    allocation_alignment: u64,
    all_bindings: &[Gfx942DispatchBufferBindingV1],
    contract: InspectedBufferContractV1,
) -> Result<(DevicePointerPatchV1, DeviceDataEffectV1), Gfx942DispatchBindingErrorV1> {
    let access = contract
        .actual_access
        .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "missing inspected actual access",
        })?;
    if contract
        .declared_access
        .is_some_and(|declared| declared != access)
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "declared/actual access contradiction",
        });
    }
    let required_alignment =
        contract
            .pointee_alignment
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "missing inspected pointee alignment",
            })?;
    let pointer_offset = usize::try_from(contract.pointer_offset).map_err(|_| {
        Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "pointer field offset conversion",
        }
    })?;
    let pointer_end =
        pointer_offset
            .checked_add(8)
            .ok_or(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "pointer field overflow",
            })?;
    if !pointer_offset.is_multiple_of(8)
        || pointer_end > kernarg_bytes.len()
        || kernarg_bytes[pointer_offset..pointer_end] != [0; 8]
        || binding.byte_len == 0
        || required_alignment == 0
        || !required_alignment.is_power_of_two()
        || required_alignment > allocation_alignment
        || !binding.data_byte_offset.is_multiple_of(required_alignment)
        || binding
            .data_byte_offset
            .checked_add(binding.byte_len)
            .is_none_or(|end| end > allocation_bytes)
        || all_bindings.iter().any(|other| {
            other != binding
                && other.data_index == binding.data_index
                && ranges_overlap_u64(
                    other.data_byte_offset,
                    other.byte_len,
                    binding.data_byte_offset,
                    binding.byte_len,
                )
        })
    {
        return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet,
            detail: "device buffer range or alias",
        });
    }
    let effect = match access {
        ArgumentAccess::ReadOnly => DeviceDataEffectV1::ReadOnly,
        ArgumentAccess::WriteOnly => DeviceDataEffectV1::WriteOnly,
        ArgumentAccess::ReadWrite => DeviceDataEffectV1::ReadWrite,
    };
    Ok((
        DevicePointerPatchV1::new(
            pointer_offset,
            binding.data_index,
            binding.data_byte_offset,
            binding.byte_len,
            required_alignment,
        ),
        effect,
    ))
}

fn merge_effect(
    existing: Option<DeviceDataEffectV1>,
    next: DeviceDataEffectV1,
) -> DeviceDataEffectV1 {
    match existing {
        None => next,
        Some(existing) if existing == next => next,
        Some(_) => DeviceDataEffectV1::ReadWrite,
    }
}

fn validate_initialization_premises(
    effects: &[Option<DeviceDataEffectV1>],
    initialized: &[bool],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if effects.len() != initialized.len() {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index: effects.len().min(initialized.len()),
            detail: "initialization/effect cardinality",
        });
    }
    for (index, effect) in effects.iter().enumerate() {
        if effect.is_some_and(DeviceDataEffectV1::reads) && !initialized[index] {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "inspected read requires sealed initialized storage",
            });
        }
    }
    Ok(())
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|end| end & !(alignment - 1))
}

fn ranges_overlap_u64(left: u64, left_len: u64, right: u64, right_len: u64) -> bool {
    let Some(left_end) = left.checked_add(left_len) else {
        return true;
    };
    let Some(right_end) = right.checked_add(right_len) else {
        return true;
    };
    left < right_end && right < left_end
}

fn validate_packet_count<const N: usize>() -> Result<(), Gfx942DispatchBindingErrorV1> {
    if N == 0 {
        return Err(Gfx942DispatchBindingErrorV1::ZeroPacketCount);
    }
    if N > AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize {
        return Err(Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum {
            requested: N,
            maximum: AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize,
        });
    }
    Ok(())
}

pub(super) fn validate_fixed_batch_ring<const N: usize>(
    ring_bytes: u32,
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    validate_packet_count::<N>()?;
    let capacity = AqlRingCapacityV1::from_ring_bytes(ring_bytes)
        .map_err(|_| Gfx942DispatchBindingErrorV1::RingCapacity {
            requested: N,
            capacity: 0,
        })?
        .packets();
    if N > capacity as usize {
        return Err(Gfx942DispatchBindingErrorV1::RingCapacity {
            requested: N,
            capacity,
        });
    }
    Ok(())
}

fn validate_data_inputs(
    data: &[DeviceDataAllocationInputV1],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    if data.is_empty() || data.len() > MAX_DISPATCH_DATA_LEASES_V1 {
        return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
            requested: data.len(),
            maximum: MAX_DISPATCH_DATA_LEASES_V1,
        });
    }
    for (index, input) in data.iter().enumerate() {
        let premise = input.premise;
        if premise.role_identity == [0; 32] {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "zero role identity",
            });
        }
        if input.requested_bytes == 0
            || input.alignment == 0
            || !input.alignment.is_power_of_two()
            || input.alignment > 4096
            || premise.valid_bytes == 0
            || premise.valid_bytes > input.requested_bytes
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "valid byte extent",
            });
        }
        if premise.effect.reads() {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "read requires authenticated initialized-content authority",
            });
        }
        if data[..index]
            .iter()
            .any(|prior| prior.premise.role_identity == premise.role_identity)
        {
            return Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index,
                detail: "role identity alias",
            });
        }
    }
    Ok(())
}

fn validate_kernargs<const N: usize>(
    kernargs: &[TypedKernargImageV1; N],
    expected_bytes: usize,
    data: &[DeviceDataAllocationInputV1],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    let mut referenced = vec![false; data.len()];
    for (packet, kernarg) in kernargs.iter().enumerate() {
        if kernarg.layout_identity == [0; 32] || kernarg.bytes.len() != expected_bytes {
            return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                packet,
                detail: "typed layout identity or size",
            });
        }
        for (patch_index, patch) in kernarg.device_pointers.iter().enumerate() {
            let end = patch.byte_offset.checked_add(8).ok_or(
                Gfx942DispatchBindingErrorV1::InvalidKernarg {
                    packet,
                    detail: "pointer field overflow",
                },
            )?;
            if !patch.byte_offset.is_multiple_of(8)
                || end > kernarg.bytes.len()
                || kernarg.bytes[patch.byte_offset..end] != [0; 8]
                || patch.required_bytes == 0
                || patch.required_alignment == 0
                || !patch.required_alignment.is_power_of_two()
                || patch.data_index >= data.len()
                || !patch
                    .data_byte_offset
                    .is_multiple_of(patch.required_alignment)
                || patch.required_alignment > data[patch.data_index].alignment
                || patch
                    .data_byte_offset
                    .checked_add(patch.required_bytes)
                    .is_none_or(|end| end > data[patch.data_index].premise.valid_bytes)
                || kernarg.device_pointers[..patch_index]
                    .iter()
                    .any(|prior| ranges_overlap_usize(prior.byte_offset, 8, patch.byte_offset, 8))
            {
                return Err(Gfx942DispatchBindingErrorV1::InvalidKernarg {
                    packet,
                    detail: "device pointer patch",
                });
            }
            referenced[patch.data_index] = true;
        }
    }
    if let Some(index) = referenced.iter().position(|referenced| !referenced) {
        return Err(Gfx942DispatchBindingErrorV1::InvalidData {
            index,
            detail: "lease not referenced by kernarg",
        });
    }
    Ok(())
}

fn validate_geometry<const N: usize>(
    resources: fe2o3_amdhsa_loader::SelectedKernelResourceBindingV1,
    geometry: &[DispatchGeometryV1; N],
) -> Result<(), Gfx942DispatchBindingErrorV1> {
    for (packet, dispatch) in geometry.iter().enumerate() {
        let observed_workgroup = dispatch.geometry.workgroup();
        let workgroup = observed_workgroup.map(u32::from);
        let grid = dispatch.geometry.grid();
        let flat = u64::from(workgroup[0])
            .checked_mul(u64::from(workgroup[1]))
            .and_then(|xy| xy.checked_mul(u64::from(workgroup[2])))
            .ok_or(Gfx942DispatchBindingErrorV1::Geometry {
                packet,
                detail: "workgroup product",
            })?;
        if flat > u64::from(resources.max_flat_workgroup_size())
            || resources
                .required_workgroup_size()
                .is_some_and(|required| required != workgroup)
        {
            return Err(Gfx942DispatchBindingErrorV1::Geometry {
                packet,
                detail: "workgroup resource contract",
            });
        }
        for dimension in 0..3 {
            if resources.max_workgroups()[dimension]
                .is_some_and(|maximum| grid[dimension].div_ceil(workgroup[dimension]) > maximum)
            {
                return Err(Gfx942DispatchBindingErrorV1::Geometry {
                    packet,
                    detail: "workgroup count",
                });
            }
        }
        u64::from(dispatch.dynamic_group_segment_bytes)
            .checked_add(resources.group_segment_fixed_size())
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or(Gfx942DispatchBindingErrorV1::Geometry {
                packet,
                detail: "group segment size",
            })?;
    }
    Ok(())
}

fn ranges_overlap_usize(left: usize, left_len: usize, right: usize, right_len: usize) -> bool {
    let Some(left_end) = left.checked_add(left_len) else {
        return true;
    };
    let Some(right_end) = right.checked_add(right_len) else {
        return true;
    };
    left < right_end && right < left_end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn premise(seed: u8, effect: DeviceDataEffectV1) -> DeviceDataPremiseV1 {
        DeviceDataPremiseV1::new([seed; 32], 4096, effect)
    }

    fn typed(bytes: usize, patches: impl Into<Box<[DevicePointerPatchV1]>>) -> TypedKernargImageV1 {
        TypedKernargImageV1::new(
            [0x51; 32],
            vec![0; bytes].into_boxed_slice(),
            patches.into(),
        )
    }

    // Pure preflight uses the same layout values as real C3 leases. The native
    // lifecycle and fault boundaries are covered by shared_memory's backend
    // fault matrix; these tests target dispatch-specific mutation ordering.
    fn fake_input(seed: u64, premise: DeviceDataPremiseV1) -> DeviceDataAllocationInputV1 {
        let _ = seed;
        DeviceDataAllocationInputV1 {
            requested_bytes: 4096,
            alignment: 4096,
            premise,
        }
    }

    #[test]
    fn manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(rendered, GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1);
    }

    fn inspected(
        pointer_offset: u64,
        actual_access: Option<ArgumentAccess>,
        pointee_alignment: Option<u64>,
    ) -> InspectedBufferContractV1 {
        InspectedBufferContractV1 {
            pointer_offset,
            declared_access: actual_access,
            actual_access,
            pointee_alignment,
        }
    }

    #[test]
    fn public_buffer_contract_rejects_pointer_range_alignment_and_alias_drift() {
        let binding = Gfx942DispatchBufferBindingV1::new(0, 0, 0, 64);
        let bytes = [0u8; 32];
        let (patch, effect) = validate_inspected_buffer_contract(
            0,
            &bytes,
            &binding,
            4096,
            4096,
            &[binding],
            inspected(8, Some(ArgumentAccess::ReadOnly), Some(16)),
        )
        .unwrap();
        assert_eq!(patch.byte_offset, 8);
        assert_eq!(effect, DeviceDataEffectV1::ReadOnly);

        let mut nonzero = bytes;
        nonzero[8] = 1;
        assert!(
            validate_inspected_buffer_contract(
                0,
                &nonzero,
                &binding,
                4096,
                4096,
                &[binding],
                inspected(8, Some(ArgumentAccess::ReadOnly), Some(16)),
            )
            .is_err()
        );

        let overflow = Gfx942DispatchBufferBindingV1::new(0, 0, 4080, 32);
        assert!(
            validate_inspected_buffer_contract(
                0,
                &bytes,
                &overflow,
                4096,
                4096,
                &[overflow],
                inspected(8, Some(ArgumentAccess::WriteOnly), Some(16)),
            )
            .is_err()
        );

        let misaligned = Gfx942DispatchBufferBindingV1::new(0, 0, 4, 64);
        assert!(
            validate_inspected_buffer_contract(
                0,
                &bytes,
                &misaligned,
                4096,
                4096,
                &[misaligned],
                inspected(8, Some(ArgumentAccess::WriteOnly), Some(16)),
            )
            .is_err()
        );

        let alias = Gfx942DispatchBufferBindingV1::new(1, 0, 32, 64);
        assert!(
            validate_inspected_buffer_contract(
                0,
                &bytes,
                &binding,
                4096,
                4096,
                &[binding, alias],
                inspected(8, Some(ArgumentAccess::ReadOnly), Some(16)),
            )
            .is_err()
        );
    }

    #[test]
    fn public_buffer_contract_requires_inspected_access_and_alignment() {
        let binding = Gfx942DispatchBufferBindingV1::new(0, 0, 0, 64);
        let bytes = [0u8; 32];
        assert!(
            validate_inspected_buffer_contract(
                0,
                &bytes,
                &binding,
                4096,
                4096,
                &[binding],
                inspected(8, None, Some(16)),
            )
            .is_err()
        );
        assert!(
            validate_inspected_buffer_contract(
                0,
                &bytes,
                &binding,
                4096,
                4096,
                &[binding],
                inspected(8, Some(ArgumentAccess::ReadOnly), None),
            )
            .is_err()
        );
        assert!(
            validate_inspected_buffer_contract(
                0,
                &bytes,
                &binding,
                4096,
                4096,
                &[binding],
                InspectedBufferContractV1 {
                    pointer_offset: 8,
                    declared_access: Some(ArgumentAccess::WriteOnly),
                    actual_access: Some(ArgumentAccess::ReadOnly),
                    pointee_alignment: Some(16),
                },
            )
            .is_err()
        );
        assert!(
            validate_inspected_buffer_contract(
                0,
                &bytes,
                &binding,
                4096,
                4096,
                &[binding],
                inspected(4, Some(ArgumentAccess::ReadOnly), Some(16)),
            )
            .is_err()
        );
    }

    #[test]
    fn public_read_effect_requires_sealed_initialization() {
        assert!(
            validate_initialization_premises(&[Some(DeviceDataEffectV1::WriteOnly)], &[false],)
                .is_ok()
        );
        assert!(
            validate_initialization_premises(&[Some(DeviceDataEffectV1::ReadOnly)], &[true],)
                .is_ok()
        );
        assert!(matches!(
            validate_initialization_premises(&[Some(DeviceDataEffectV1::ReadWrite)], &[false],),
            Err(Gfx942DispatchBindingErrorV1::InvalidData { index: 0, .. })
        ));
        assert_eq!(
            merge_effect(
                Some(DeviceDataEffectV1::WriteOnly),
                DeviceDataEffectV1::ReadOnly,
            ),
            DeviceDataEffectV1::ReadWrite
        );
    }

    #[test]
    fn packet_and_data_bounds_are_exact() {
        assert_eq!(
            validate_packet_count::<0>().unwrap_err().to_string(),
            "ZeroPacketCount"
        );
        assert!(validate_packet_count::<1>().is_ok());
        assert!(validate_packet_count::<1024>().is_ok());
        assert!(matches!(
            validate_packet_count::<1025>(),
            Err(Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum { .. })
        ));
        assert!(matches!(
            validate_data_inputs(&[]),
            Err(Gfx942DispatchBindingErrorV1::DataLeaseCount { .. })
        ));
        let sixteen: Vec<_> = (1..=16)
            .map(|seed| fake_input(seed, premise(seed as u8, DeviceDataEffectV1::WriteOnly)))
            .collect();
        assert!(validate_data_inputs(&sixteen).is_ok());
        let seventeen: Vec<_> = (1..=17)
            .map(|seed| fake_input(seed, premise(seed as u8, DeviceDataEffectV1::WriteOnly)))
            .collect();
        assert!(matches!(
            validate_data_inputs(&seventeen),
            Err(Gfx942DispatchBindingErrorV1::DataLeaseCount { .. })
        ));
    }

    #[test]
    fn fixed_batch_ring_must_cover_every_packet_before_native_preparation() {
        assert!(validate_fixed_batch_ring::<768>(65_536).is_ok());
        assert!(validate_fixed_batch_ring::<1024>(65_536).is_ok());
        assert!(matches!(
            validate_fixed_batch_ring::<1024>(32_768),
            Err(Gfx942DispatchBindingErrorV1::RingCapacity {
                requested: 1024,
                capacity: 512,
            })
        ));
        assert!(matches!(
            validate_fixed_batch_ring::<1025>(131_072),
            Err(Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum { .. })
        ));
    }

    #[test]
    fn data_premises_reject_uninitialized_reads_and_identity_aliases() {
        for effect in [DeviceDataEffectV1::ReadOnly, DeviceDataEffectV1::ReadWrite] {
            assert!(matches!(
                validate_data_inputs(&[fake_input(1, premise(1, effect))]),
                Err(Gfx942DispatchBindingErrorV1::InvalidData {
                    detail: "read requires authenticated initialized-content authority",
                    ..
                })
            ));
        }
        assert!(
            validate_data_inputs(&[fake_input(1, premise(1, DeviceDataEffectV1::WriteOnly))])
                .is_ok()
        );
        assert!(matches!(
            validate_data_inputs(&[
                fake_input(1, premise(7, DeviceDataEffectV1::WriteOnly)),
                fake_input(2, premise(7, DeviceDataEffectV1::WriteOnly)),
            ]),
            Err(Gfx942DispatchBindingErrorV1::InvalidData {
                detail: "role identity alias",
                ..
            })
        ));
    }

    #[test]
    fn typed_pointer_layout_is_complete_bounded_and_nonoverlapping() {
        let data = [
            fake_input(1, premise(1, DeviceDataEffectV1::WriteOnly)),
            fake_input(2, premise(2, DeviceDataEffectV1::WriteOnly)),
        ];
        let valid = typed(
            32,
            vec![
                DevicePointerPatchV1::new(0, 0, 0, 4096, 8),
                DevicePointerPatchV1::new(8, 1, 0, 4096, 8),
            ],
        );
        assert!(validate_kernargs(&[valid], 32, &data).is_ok());

        let cases = [
            typed(32, vec![DevicePointerPatchV1::new(1, 0, 0, 8, 8)]),
            typed(32, vec![DevicePointerPatchV1::new(32, 0, 0, 8, 8)]),
            typed(32, vec![DevicePointerPatchV1::new(0, 2, 0, 8, 8)]),
            typed(32, vec![DevicePointerPatchV1::new(0, 0, 4090, 8, 8)]),
            typed(
                32,
                vec![
                    DevicePointerPatchV1::new(0, 0, 0, 8, 8),
                    DevicePointerPatchV1::new(0, 1, 0, 8, 8),
                ],
            ),
        ];
        for invalid in cases {
            assert!(matches!(
                validate_kernargs(&[invalid], 32, &data),
                Err(Gfx942DispatchBindingErrorV1::InvalidKernarg { .. })
            ));
        }
    }

    #[test]
    fn every_data_lease_must_be_referenced_by_every_batch_shape() {
        let data = [
            fake_input(1, premise(1, DeviceDataEffectV1::WriteOnly)),
            fake_input(2, premise(2, DeviceDataEffectV1::WriteOnly)),
        ];
        let only_first = typed(16, vec![DevicePointerPatchV1::new(0, 0, 0, 8, 8)]);
        assert!(matches!(
            validate_kernargs(&[only_first], 16, &data),
            Err(Gfx942DispatchBindingErrorV1::InvalidData {
                index: 1,
                detail: "lease not referenced by kernarg"
            })
        ));
    }

    #[test]
    fn owner_phase_is_linear_and_terminal_poison_is_sticky() {
        let mut owner = DispatchGenerationOwnerV1::new();
        let generation = owner.next().unwrap();
        assert_eq!(generation, 1);
        assert!(owner.active().is_err());
        owner.commit_begin(generation);
        assert_eq!(owner.active().unwrap(), generation);
        assert!(owner.cancel(generation + 1).is_err());
        owner.complete(generation).unwrap();
        assert!(owner.recycle(generation + 1).is_err());
        owner.recycle(generation).unwrap();
        assert!(owner.ensure_prepared().is_ok());
        assert_eq!(owner.returned_generation().unwrap(), generation);
        owner.poison();
        assert!(matches!(
            owner.next(),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
    }

    #[test]
    fn recycled_queue_can_admit_a_different_second_fixed_batch_generation() {
        let queue_generation = 19u64;
        let first = Gfx942FixedDispatchPacketV1::new(
            0,
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            vec![0, 0, 0, 1].into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        );
        let second = [
            Gfx942FixedDispatchPacketV1::new(
                1,
                AqlDispatchGeometryV1::new([128, 1, 1], [64, 1, 1]).unwrap(),
                256,
                vec![0, 0, 0, 2].into_boxed_slice(),
                Vec::new().into_boxed_slice(),
            ),
            Gfx942FixedDispatchPacketV1::new(
                0,
                AqlDispatchGeometryV1::new([32, 2, 1], [32, 1, 1]).unwrap(),
                0,
                vec![0, 0, 0, 3].into_boxed_slice(),
                Vec::new().into_boxed_slice(),
            ),
        ];
        assert_ne!(first.program_index, second[0].program_index);
        assert_ne!(first.geometry, second[0].geometry);
        assert_ne!(first.kernarg_bytes, second[0].kernarg_bytes);
        validate_fixed_batch_ring::<1>(65_536).unwrap();
        validate_fixed_batch_ring::<2>(65_536).unwrap();

        let fully_initialized = true;
        let mut owner = DispatchGenerationOwnerV1::new();
        let first_generation = owner.next().unwrap();
        owner.commit_begin(first_generation);
        owner.complete(first_generation).unwrap();
        owner.recycle(first_generation).unwrap();
        let second_generation = owner.next().unwrap();
        owner.commit_begin(second_generation);
        owner.complete(second_generation).unwrap();
        owner.recycle(second_generation).unwrap();

        assert_eq!(first_generation, 1);
        assert_eq!(second_generation, 2);
        assert_eq!(queue_generation, 19);
        assert!(fully_initialized);
        assert_eq!(second.len(), 2);
    }

    #[test]
    fn stale_and_double_use_transitions_never_mutate_generation_state() {
        let mut owner = DispatchGenerationOwnerV1::new();
        let generation = owner.next().unwrap();
        owner.commit_begin(generation);
        for stale in [0, generation + 1, u64::MAX] {
            let before = owner;
            assert!(matches!(
                owner.cancel(stale),
                Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
            ));
            assert_eq!(owner, before);
            assert!(matches!(
                owner.complete(stale),
                Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
            ));
            assert_eq!(owner, before);
            assert!(matches!(
                owner.recycle(stale),
                Err(Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)
            ));
            assert_eq!(owner, before);
        }
        owner.complete(generation).unwrap();
        let completed = owner;
        assert!(owner.complete(generation).is_err());
        assert_eq!(owner, completed);
        owner.recycle(generation).unwrap();
        let recycled = owner;
        assert!(owner.recycle(generation).is_err());
        assert_eq!(owner, recycled);
        assert_eq!(owner.returned_generation().unwrap(), generation);

        let next = owner.next().unwrap();
        owner.commit_begin(next);
        assert!(owner.returned_generation().is_err());
        owner.cancel(next).unwrap();
        assert!(owner.returned_generation().is_err());
    }

    #[test]
    fn data_return_requires_exact_completion_and_recycle() {
        let mut owner = DispatchGenerationOwnerV1::new();
        assert!(owner.returned_generation().is_err());

        let generation = owner.next().unwrap();
        owner.commit_begin(generation);
        assert!(owner.returned_generation().is_err());
        owner.complete(generation).unwrap();
        assert!(owner.returned_generation().is_err());
        assert!(owner.recycle(generation + 1).is_err());
        assert!(owner.returned_generation().is_err());
        owner.recycle(generation).unwrap();
        assert_eq!(owner.returned_generation().unwrap(), generation);
    }

    #[test]
    fn exhaustion_and_poison_from_each_phase_are_terminal_and_fail_closed() {
        let exhausted = DispatchGenerationOwnerV1 {
            next_generation: u64::MAX,
            phase: DispatchOwnerPhaseV1::Prepared,
            recycled_generation: None,
        };
        let before = exhausted;
        assert!(matches!(
            exhausted.next(),
            Err(Gfx942DispatchBindingErrorV1::GenerationExhausted)
        ));
        assert_eq!(exhausted, before);

        for phase in [
            DispatchOwnerPhaseV1::Prepared,
            DispatchOwnerPhaseV1::InFlight { generation: 7 },
            DispatchOwnerPhaseV1::Completed { generation: 7 },
        ] {
            let mut owner = DispatchGenerationOwnerV1 {
                next_generation: 8,
                phase,
                recycled_generation: None,
            };
            owner.poison();
            assert_eq!(owner.phase, DispatchOwnerPhaseV1::Poisoned);
            assert!(matches!(
                owner.next(),
                Err(Gfx942DispatchBindingErrorV1::Poisoned)
            ));
            assert!(matches!(
                owner.cancel(7),
                Err(Gfx942DispatchBindingErrorV1::Poisoned)
            ));
            assert!(matches!(
                owner.complete(7),
                Err(Gfx942DispatchBindingErrorV1::Poisoned)
            ));
            assert!(matches!(
                owner.recycle(7),
                Err(Gfx942DispatchBindingErrorV1::Poisoned)
            ));
        }
    }
}

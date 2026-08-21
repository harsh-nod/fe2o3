//! Private dispatch binding for the retained gfx942 compute-AQL queue.
//!
//! This module closes host-side identity, layout, ownership, and lifetime
//! composition. It deliberately has no public constructor or submission
//! method: device-data initialization/effect premises and native execution
//! semantics remain reviewed integration obligations.

#![allow(dead_code)]

use core::fmt;

use fe2o3_amdhsa_loader::{KernelIdentityInputsV1, ValidatedKernelEnvelope};
use fe2o3_aql::{AQL_MAX_BATCH_PACKETS_V1, AqlDispatchGeometryV1, ObservedGpuAddressV1};
use fe2o3_runtime_model::{MemoryMappingKeyV1, QueueKeyV1};
use sha2::{Digest, Sha256};

use super::completion::{
    CompletionDispatchGenerationBindingV1, CompletionPacketTemplateV1, Gfx942CompletedBatchV1,
    Gfx942CompletionBatchV1, Gfx942CompletionErrorV1, Gfx942CompletionPollV1,
};
use crate::MemorySessionError;
use crate::shared_memory::{
    AqlDispatchCodeResourceRoleV1, AqlDispatchKernargResourceRoleV1, ExecutableGttV1,
    Gfx942DeviceMemoryDispatchAuthorityV1, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
    GttGpuAccessibleExecutableV1, GttGpuAccessibleMutableV1, KernargGttV1,
    SharedGttMemorySessionV1, SharedGttQueueResourceAuthorityV1,
};

pub(crate) const MAX_DISPATCH_DATA_LEASES_V1: usize = 16;
pub(crate) const MAX_DISPATCH_KERNARG_BYTES_V1: usize = 65_536;
const KERNEL_DESCRIPTOR_BYTES_V1: u64 = 64;

/// Frozen claim boundary for the private dispatch-binding slice.
pub const GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-aql-dispatch-binding-r2-v1\n",
    "target=gfx942:xnack-,COV6,one-selected-current-device-vm-and-queue-generation\n",
    "code=validated-amdhsa-kernel-envelope,content-and-selected-descriptor-identity,exact-zero-then-copy-materialization-into-owned-gtt,read-only-seal-before-map,descriptor-resolution-with-checked-relative-arithmetic\n",
    "kernarg=private-typed-complete-image,exact-selected-size-and-power-of-two-alignment,checked-nonoverlapping-8-byte-device-pointer-patches,one-owned-kernarg-gtt-arena-with-N-distinct-checked-aligned-slices,initialized-before-map\n",
    "data=1-through-16-actual-linear-c3-mapped-device-memory-leases,exact-device-vm-generation-and-whole-allocation-nonalias,checked-valid-byte-extents,write-only-until-authenticated-copy-completion-authority-exists,declared-effects-retained\n",
    "batch=1-through-256,one-code-owner,N-distinct-kernarg-owners,one-generation-bound-private-template-per-packet,C2-one-reservation-one-doorbell-and-C4-one-signal-per-packet-composition\n",
    "retention=queue-owns-code-kernarg-and-device-leases-through-exact-C4-ready-and-recycle,ordinary-destroy-releases-all,returning-destroy-requires-one-exact-recycled-generation-and-returns-actual-mapped-c3-authorities-with-owning-memory-session\n",
    "queue-transfer=ordinary-path-still-rejects-device-memory,dispatch-path-requires-exact-complete-distinct-set-of-every-live-mapped-c3-lease-before-model-mutation\n",
    "failure=all-layout-and-identity-validation-before-native-preparation;post-side-effect-failure,currentness,publication,completion,timeout,recycle-or-release-ambiguity-poisons-and-requires-teardown\n",
    "authority=crate-private-preparation-bind-submit-poll-wait-recycle,no-public-constructor,no-address-handle-pointer-fd-kernarg-byte-or-generic-launch-export\n",
    "proof=bounded-host-state-machine-and-mock-fault-tests-only,no-concrete-verus-or-machine-refinement\n",
    "contracted=code-segment-permission-refinement,implicit-kernarg-producer,cpu-gpu-coherence,firmware-dispatch-effects-and-quiescence\n",
    "excluded=public-safe-launch,public-packet-template,async-copy,initialized-content-mint,read-premise,device-address-export,alias-suballocation,peer-map,hardware-execution\n",
);

/// SHA-256 of [`GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1`].
pub const GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1: &str =
    "5f94ed69091dac7d1f405b4be9fa313878c6e9d1d0ee4783559ad4625db84d66";

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
}

/// Queue-retained real resource owner for one prepared batch shape.
pub(super) struct DispatchResourceOwnerV1 {
    code: CodeAuthority,
    code_identity: ResolvedCodeIdentityV1,
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
            || self.code_identity.mapping.allocation.vm != queue.vm
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
                CompletionPacketTemplateV1::new(
                    packet.geometry,
                    packet.private_segment_size,
                    packet.group_segment_size,
                    self.code_identity.descriptor_address,
                    packet.kernarg_address,
                    packet.kernarg_alignment,
                    CompletionDispatchGenerationBindingV1::new(
                        queue,
                        self.code_identity.mapping,
                        packet.kernarg_mapping,
                        generation,
                    ),
                )
            })
            .collect();
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
        let code = memory.unmap_executable_from_gpu(self.code.into_token())?;
        memory.release_executable(code)?;
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
        let code = memory.unmap_executable_from_gpu(self.code.into_token())?;
        memory.release_executable(code)?;
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
        });
    }

    Ok(DispatchResourceOwnerV1 {
        code,
        code_identity,
        kernarg,
        packets,
        data: data_authorities,
        data_premises,
        generation: DispatchGenerationOwnerV1::new(),
    })
}

fn validate_packet_count<const N: usize>() -> Result<(), Gfx942DispatchBindingErrorV1> {
    if N == 0 {
        return Err(Gfx942DispatchBindingErrorV1::ZeroPacketCount);
    }
    if N > AQL_MAX_BATCH_PACKETS_V1 as usize {
        return Err(Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum {
            requested: N,
            maximum: AQL_MAX_BATCH_PACKETS_V1 as usize,
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

    #[test]
    fn packet_and_data_bounds_are_exact() {
        assert_eq!(
            validate_packet_count::<0>().unwrap_err().to_string(),
            "ZeroPacketCount"
        );
        assert!(validate_packet_count::<1>().is_ok());
        assert!(validate_packet_count::<256>().is_ok());
        assert!(matches!(
            validate_packet_count::<257>(),
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

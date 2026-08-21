//! Private, bounded completion authority for the retained compute-AQL queue.
//!
//! Every packet in a batch receives one distinct ROCr user signal from one
//! coherent GTT arena. Numeric addresses remain inside this module. The host
//! state machine retains exact queue, signal-allocation, kernarg-allocation,
//! and data-allocation generations until all signal values have been observed
//! with acquire ordering and the batch has been explicitly recycled.

use core::fmt;

use fe2o3_aql::{
    AMD_SIGNAL_ALIGNMENT_V1, AMD_SIGNAL_BYTES_V1, AQL_MAX_BATCH_PACKETS_V1,
    AmdBusyCompletionSignalV1, AqlCompletionObservationV1, AqlDispatchGeometryV1,
    AqlDispatchPacketError, AqlKernelDispatchPacketV1, AqlPreparedKernelDispatchBatchErrorV1,
    AqlPreparedKernelDispatchBatchV1, AqlPreparedKernelDispatchV1, ObservedGpuAddressV1,
};
use fe2o3_runtime_model::{MemoryMappingKeyV1, QueueKeyV1};

use crate::shared_memory::SharedGttMappedResourceFactsV1;

pub(crate) const COMPLETION_SIGNAL_CAPACITY_V1: usize = AQL_MAX_BATCH_PACKETS_V1 as usize;
pub(crate) const COMPLETION_SIGNAL_ARENA_BYTES_V1: usize =
    COMPLETION_SIGNAL_CAPACITY_V1 * AMD_SIGNAL_BYTES_V1;
pub(super) const MAX_COMPLETION_POLL_ATTEMPTS_V1: u32 = 1_000_000;

/// Canonical claim boundary for the private completion-signal slice.
pub const GFX942_AQL_COMPLETION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-aql-completion-r1-v1\n",
    "aql_dispatch_schema_sha256=b691e0df36e2c1f0695f49a19d49d3fbbe4380e8e9999b01368df02783952edf\n",
    "arena=one-host-visible-coherent-gtt-allocation,16384-bytes,256-distinct-64-byte-aligned-user-signals\n",
    "batch=1-through-256,one-unique-signal-per-packet,no-aggregate-alias\n",
    "initialization=typed-amd-busy-signal-construction,kind-user-1,value-pending-1,event-fields-zero,before-gpu-map\n",
    "binding=crate-private-packet-construction,no-public-signal-address,exact-queue-vm-signal-kernarg-and-data-allocation-generations-retained\n",
    "observation=bounded-busy-poll,atomic-i64-acquire,all-signals-zero-before-ready,unexpected-value-is-fault\n",
    "recycle=only-after-exact-all-signal-completion,atomic-i64-release-reset-to-pending,checked-slot-generation-increment\n",
    "failure=currentness-native-observation-unexpected-value-timeout-invalid-poll-bound-generation-exhaustion-or-reset-ambiguity-poisons-owner-and-queue;teardown-required\n",
    "release=queue-destroy-first,only-when-every-batch-was-completed-and-recycled,explicit-unmap-and-free,no-drop-native-effects\n",
    "proof=host-state-machine-and-mock-fault-tests-only,cpu-gpu-atomic-coherence-device-write-visibility-firmware-signal-and-quiescence-refinement-contracted\n",
    "excluded=public-safe-launch,code-object-authority,kernarg-or-device-allocation-liveness-mint,copy,alias-proof,hardware-execution,ioctl-validation\n",
);

/// SHA-256 of [`GFX942_AQL_COMPLETION_MANIFEST_V1`].
pub const GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1: &str =
    "639120a81cd1bba9a94ca1a3550c7e3664de1e23e0d6c812f10a9283444b69dd";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionOwnerPhaseV1 {
    Ready,
    Poisoned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionSlotPhaseV1 {
    Available,
    Bound { batch_id: u64 },
    Published { batch_id: u64 },
    Completed { batch_id: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletionSlotRecordV1 {
    generation: u64,
    phase: CompletionSlotPhaseV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompletionDispatchGenerationBindingV1 {
    queue: QueueKeyV1,
    kernarg: MemoryMappingKeyV1,
    data: MemoryMappingKeyV1,
}

impl CompletionDispatchGenerationBindingV1 {
    #[allow(dead_code)]
    pub(crate) const fn new(
        queue: QueueKeyV1,
        kernarg: MemoryMappingKeyV1,
        data: MemoryMappingKeyV1,
    ) -> Self {
        Self {
            queue,
            kernarg,
            data,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompletionPacketTemplateV1 {
    geometry: AqlDispatchGeometryV1,
    private_segment_size: u32,
    group_segment_size: u32,
    kernel_object: ObservedGpuAddressV1,
    kernarg_address: ObservedGpuAddressV1,
    kernarg_alignment: u64,
    generations: CompletionDispatchGenerationBindingV1,
}

impl CompletionPacketTemplateV1 {
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) const fn new(
        geometry: AqlDispatchGeometryV1,
        private_segment_size: u32,
        group_segment_size: u32,
        kernel_object: ObservedGpuAddressV1,
        kernarg_address: ObservedGpuAddressV1,
        kernarg_alignment: u64,
        generations: CompletionDispatchGenerationBindingV1,
    ) -> Self {
        Self {
            geometry,
            private_segment_size,
            group_segment_size,
            kernel_object,
            kernarg_address,
            kernarg_alignment,
            generations,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletionSlotLeaseV1 {
    index: u32,
    generation: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct CompletionBatchRetentionV1<const N: usize> {
    batch_id: u64,
    queue: QueueKeyV1,
    signal_mapping: MemoryMappingKeyV1,
    slots: [CompletionSlotLeaseV1; N],
    dispatches: [CompletionDispatchGenerationBindingV1; N],
    last_packet_id: Option<u64>,
}

pub(super) struct BoundCompletionBatchV1<const N: usize> {
    packets: AqlPreparedKernelDispatchBatchV1<N>,
    retention: CompletionBatchRetentionV1<N>,
}

impl<const N: usize> BoundCompletionBatchV1<N> {
    pub(super) fn into_parts(
        self,
    ) -> (
        AqlPreparedKernelDispatchBatchV1<N>,
        CompletionBatchRetentionV1<N>,
    ) {
        (self.packets, self.retention)
    }
}

/// Linear authority for one published completion batch.
///
/// This type is not `Clone` or `Copy`, has no public constructor, and exposes
/// neither signal addresses nor retained generation keys.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942CompletionBatchV1;
///
/// fn consume<const N: usize>(_: Gfx942CompletionBatchV1<N>) {}
/// fn cannot_observe_twice<const N: usize>(batch: Gfx942CompletionBatchV1<N>) {
///     consume(batch);
///     consume(batch);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942CompletionBatchV1;
///
/// fn cannot_clone<const N: usize>(batch: &Gfx942CompletionBatchV1<N>) {
///     let _duplicate: Gfx942CompletionBatchV1<N> = batch.clone();
/// }
/// ```
#[must_use = "a published completion batch must be observed or retained for teardown"]
pub struct Gfx942CompletionBatchV1<const N: usize> {
    retention: CompletionBatchRetentionV1<N>,
}

impl<const N: usize> fmt::Debug for Gfx942CompletionBatchV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942CompletionBatchV1")
            .field("packet_count", &N)
            .finish_non_exhaustive()
    }
}

/// Linear evidence that every signal in one exact batch was acquired as zero.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942CompletedBatchV1;
///
/// fn recycle<const N: usize>(_: Gfx942CompletedBatchV1<N>) {}
/// fn cannot_recycle_twice<const N: usize>(batch: Gfx942CompletedBatchV1<N>) {
///     recycle(batch);
///     recycle(batch);
/// }
/// ```
#[must_use = "completed signal slots must be explicitly recycled"]
pub struct Gfx942CompletedBatchV1<const N: usize> {
    retention: CompletionBatchRetentionV1<N>,
}

impl<const N: usize> fmt::Debug for Gfx942CompletedBatchV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942CompletedBatchV1")
            .field("packet_count", &N)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum Gfx942CompletionPollV1<const N: usize> {
    Pending(Gfx942CompletionBatchV1<N>),
    Ready(Gfx942CompletedBatchV1<N>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompletionRecycleObservationV1 {
    packet_count: u16,
}

impl Gfx942CompletionRecycleObservationV1 {
    pub const fn packet_count(self) -> u16 {
        self.packet_count
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Gfx942CompletionErrorV1 {
    InvalidArena(&'static str),
    ZeroPacketCount,
    PacketCountExceedsMaximum { requested: usize, maximum: usize },
    InsufficientSignals,
    BatchIdentityExhausted,
    SignalGenerationExhausted,
    WrongQueueGeneration,
    WrongVmGeneration,
    StaleBatchGeneration,
    Poisoned,
    Initialization,
    PacketBinding(AqlDispatchPacketError),
    BatchConstruction(AqlPreparedKernelDispatchBatchErrorV1),
    Currentness,
    Observation,
    Fault { slot: u32, value: i64 },
    InvalidPollBound { requested: u32, maximum: u32 },
    Timeout { polls: u32 },
    Recycle,
    BatchStillRetained,
}

impl fmt::Display for Gfx942CompletionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Gfx942CompletionErrorV1 {}

pub(super) trait NativeCompletionSignalBackendV1 {
    fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1>;
    fn observe_acquire(
        &mut self,
        slot_index: u32,
    ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1>;
    fn reset_pending_release(&mut self, slot_index: u32) -> Result<(), Gfx942CompletionErrorV1>;
}

pub(super) struct CompletionSignalArenaOwnerV1 {
    queue: QueueKeyV1,
    signal_mapping: MemoryMappingKeyV1,
    gpu_base: u64,
    next_batch_id: u64,
    slots: [CompletionSlotRecordV1; COMPLETION_SIGNAL_CAPACITY_V1],
    phase: CompletionOwnerPhaseV1,
}

impl CompletionSignalArenaOwnerV1 {
    pub(super) fn new(
        queue: QueueKeyV1,
        facts: &SharedGttMappedResourceFactsV1,
    ) -> Result<Self, Gfx942CompletionErrorV1> {
        if facts.mapping().allocation.vm != queue.vm
            || facts.logical_bytes() != COMPLETION_SIGNAL_ARENA_BYTES_V1
            || facts.gpu_va_bytes() != COMPLETION_SIGNAL_ARENA_BYTES_V1 as u64
            || facts
                .checked_gpu_subrange(
                    0,
                    COMPLETION_SIGNAL_ARENA_BYTES_V1 as u64,
                    AMD_SIGNAL_ALIGNMENT_V1 as u64,
                )
                .is_none()
        {
            return Err(Gfx942CompletionErrorV1::InvalidArena(
                "completion signal geometry or VM",
            ));
        }
        Ok(Self {
            queue,
            signal_mapping: facts.mapping(),
            gpu_base: facts.gpu_va(),
            next_batch_id: 1,
            slots: [CompletionSlotRecordV1 {
                generation: 1,
                phase: CompletionSlotPhaseV1::Available,
            }; COMPLETION_SIGNAL_CAPACITY_V1],
            phase: CompletionOwnerPhaseV1::Ready,
        })
    }

    pub(super) fn bind_batch<const N: usize>(
        &mut self,
        templates: [CompletionPacketTemplateV1; N],
    ) -> Result<BoundCompletionBatchV1<N>, Gfx942CompletionErrorV1> {
        self.require_ready()?;
        validate_packet_count::<N>()?;
        let next_batch_id = self
            .next_batch_id
            .checked_add(1)
            .ok_or(Gfx942CompletionErrorV1::BatchIdentityExhausted)?;
        let available: Vec<usize> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, record)| {
                (record.phase == CompletionSlotPhaseV1::Available).then_some(index)
            })
            .take(N)
            .collect();
        if available.len() != N {
            return Err(Gfx942CompletionErrorV1::InsufficientSignals);
        }

        let mut prepared = Vec::<AqlPreparedKernelDispatchV1>::with_capacity(N);
        for (template, slot_index) in templates.iter().zip(&available) {
            self.validate_dispatch_binding(template.generations)?;
            let offset = u64::try_from(*slot_index)
                .ok()
                .and_then(|index| index.checked_mul(AMD_SIGNAL_BYTES_V1 as u64))
                .ok_or(Gfx942CompletionErrorV1::InvalidArena(
                    "completion slot offset",
                ))?;
            let raw =
                self.gpu_base
                    .checked_add(offset)
                    .ok_or(Gfx942CompletionErrorV1::InvalidArena(
                        "completion slot address",
                    ))?;
            let signal = ObservedGpuAddressV1::new(raw)
                .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("completion address"))?;
            prepared.push(
                AqlKernelDispatchPacketV1::new_unpublished(
                    template.geometry,
                    template.private_segment_size,
                    template.group_segment_size,
                    template.kernel_object,
                    template.kernarg_address,
                    template.kernarg_alignment,
                    signal,
                )
                .map_err(Gfx942CompletionErrorV1::PacketBinding)?,
            );
        }
        let packets: [AqlPreparedKernelDispatchV1; N] = prepared
            .try_into()
            .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("packet array conversion"))?;
        let packets = AqlPreparedKernelDispatchBatchV1::try_from_packets(packets)
            .map_err(Gfx942CompletionErrorV1::BatchConstruction)?;
        let slots = core::array::from_fn(|batch_index| {
            let index = available[batch_index];
            CompletionSlotLeaseV1 {
                index: index as u32,
                generation: self.slots[index].generation,
            }
        });
        for slot in &slots {
            self.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Bound {
                batch_id: self.next_batch_id,
            };
        }
        let dispatches = templates.map(|template| template.generations);
        let retention = CompletionBatchRetentionV1 {
            batch_id: self.next_batch_id,
            queue: self.queue,
            signal_mapping: self.signal_mapping,
            slots,
            dispatches,
            last_packet_id: None,
        };
        self.next_batch_id = next_batch_id;
        Ok(BoundCompletionBatchV1 { packets, retention })
    }

    pub(super) fn validate_bound<const N: usize>(
        &self,
        retention: &CompletionBatchRetentionV1<N>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        if retention.last_packet_id.is_some() {
            return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
        }
        self.validate_retention(retention, |batch_id| CompletionSlotPhaseV1::Bound {
            batch_id,
        })
    }

    pub(super) fn cancel_bound<const N: usize>(
        &mut self,
        retention: CompletionBatchRetentionV1<N>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        self.validate_bound(&retention)?;
        for slot in retention.slots {
            self.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Available;
        }
        Ok(())
    }

    pub(super) fn mark_published<const N: usize>(
        &mut self,
        mut retention: CompletionBatchRetentionV1<N>,
        last_packet_id: u64,
    ) -> Result<Gfx942CompletionBatchV1<N>, Gfx942CompletionErrorV1> {
        self.validate_bound(&retention)?;
        for slot in retention.slots {
            self.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Published {
                batch_id: retention.batch_id,
            };
        }
        retention.last_packet_id = Some(last_packet_id);
        Ok(Gfx942CompletionBatchV1 { retention })
    }

    pub(super) fn observe_once<const N: usize, B: NativeCompletionSignalBackendV1>(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
        backend: &mut B,
    ) -> Result<Gfx942CompletionPollV1<N>, Gfx942CompletionErrorV1> {
        self.require_ready()?;
        self.validate_published(&batch.retention)?;
        self.checked_currentness(backend)?;
        let mut pending = false;
        for slot in &batch.retention.slots {
            let observation = match backend.observe_acquire(slot.index) {
                Ok(observation) => observation,
                Err(_) => return self.poison(Gfx942CompletionErrorV1::Observation),
            };
            match observation {
                AqlCompletionObservationV1::Pending => pending = true,
                AqlCompletionObservationV1::Completed => {}
                AqlCompletionObservationV1::Unexpected(value) => {
                    return self.poison(Gfx942CompletionErrorV1::Fault {
                        slot: slot.index,
                        value,
                    });
                }
            }
        }
        self.checked_currentness(backend)?;
        if pending {
            return Ok(Gfx942CompletionPollV1::Pending(batch));
        }
        for slot in &batch.retention.slots {
            self.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Completed {
                batch_id: batch.retention.batch_id,
            };
        }
        Ok(Gfx942CompletionPollV1::Ready(Gfx942CompletedBatchV1 {
            retention: batch.retention,
        }))
    }

    pub(super) fn wait_bounded<const N: usize, B: NativeCompletionSignalBackendV1>(
        &mut self,
        mut batch: Gfx942CompletionBatchV1<N>,
        polls: u32,
        backend: &mut B,
    ) -> Result<Gfx942CompletedBatchV1<N>, Gfx942CompletionErrorV1> {
        if polls > MAX_COMPLETION_POLL_ATTEMPTS_V1 {
            return self.poison(Gfx942CompletionErrorV1::InvalidPollBound {
                requested: polls,
                maximum: MAX_COMPLETION_POLL_ATTEMPTS_V1,
            });
        }
        if polls == 0 {
            return self.poison(Gfx942CompletionErrorV1::Timeout { polls });
        }
        for _ in 0..polls {
            match self.observe_once(batch, backend)? {
                Gfx942CompletionPollV1::Pending(pending) => batch = pending,
                Gfx942CompletionPollV1::Ready(ready) => return Ok(ready),
            }
        }
        self.poison(Gfx942CompletionErrorV1::Timeout { polls })
    }

    pub(super) fn recycle<const N: usize, B: NativeCompletionSignalBackendV1>(
        &mut self,
        completed: Gfx942CompletedBatchV1<N>,
        backend: &mut B,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942CompletionErrorV1> {
        self.require_ready()?;
        self.validate_completed(&completed.retention)?;
        if completed.retention.slots.iter().any(|slot| {
            self.slots[slot.index as usize]
                .generation
                .checked_add(1)
                .is_none()
        }) {
            return self.poison(Gfx942CompletionErrorV1::SignalGenerationExhausted);
        }
        self.checked_currentness(backend)?;
        for slot in &completed.retention.slots {
            if backend.reset_pending_release(slot.index).is_err() {
                return self.poison(Gfx942CompletionErrorV1::Recycle);
            }
        }
        self.checked_currentness(backend)?;
        for slot in completed.retention.slots {
            let record = &mut self.slots[slot.index as usize];
            record.generation += 1;
            record.phase = CompletionSlotPhaseV1::Available;
        }
        Ok(Gfx942CompletionRecycleObservationV1 {
            packet_count: N as u16,
        })
    }

    pub(super) fn ensure_releasable(&self) -> Result<(), Gfx942CompletionErrorV1> {
        self.require_ready()?;
        if self
            .slots
            .iter()
            .any(|record| record.phase != CompletionSlotPhaseV1::Available)
        {
            return Err(Gfx942CompletionErrorV1::BatchStillRetained);
        }
        Ok(())
    }

    pub(super) fn poison_owner(&mut self) {
        self.phase = CompletionOwnerPhaseV1::Poisoned;
    }

    fn validate_dispatch_binding(
        &self,
        binding: CompletionDispatchGenerationBindingV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        if binding.queue != self.queue {
            return Err(Gfx942CompletionErrorV1::WrongQueueGeneration);
        }
        if binding.kernarg.allocation.vm != self.queue.vm
            || binding.data.allocation.vm != self.queue.vm
        {
            return Err(Gfx942CompletionErrorV1::WrongVmGeneration);
        }
        Ok(())
    }

    fn validate_published<const N: usize>(
        &self,
        retention: &CompletionBatchRetentionV1<N>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        if retention.last_packet_id.is_none() {
            return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
        }
        self.validate_retention(retention, |batch_id| CompletionSlotPhaseV1::Published {
            batch_id,
        })
    }

    fn validate_completed<const N: usize>(
        &self,
        retention: &CompletionBatchRetentionV1<N>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        if retention.last_packet_id.is_none() {
            return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
        }
        self.validate_retention(retention, |batch_id| CompletionSlotPhaseV1::Completed {
            batch_id,
        })
    }

    fn validate_retention<const N: usize>(
        &self,
        retention: &CompletionBatchRetentionV1<N>,
        expected: impl Fn(u64) -> CompletionSlotPhaseV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        validate_packet_count::<N>()?;
        if retention.queue != self.queue || retention.signal_mapping != self.signal_mapping {
            return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
        }
        for (batch_index, slot) in retention.slots.iter().enumerate() {
            let Some(record) = self.slots.get(slot.index as usize) else {
                return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
            };
            if record.generation != slot.generation
                || record.phase != expected(retention.batch_id)
                || retention.dispatches[batch_index].queue != retention.queue
                || retention.dispatches[batch_index].kernarg.allocation.vm != retention.queue.vm
                || retention.dispatches[batch_index].data.allocation.vm != retention.queue.vm
                || retention.slots[..batch_index]
                    .iter()
                    .any(|prior| prior.index == slot.index)
            {
                return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
            }
        }
        Ok(())
    }

    fn checked_currentness<B: NativeCompletionSignalBackendV1>(
        &mut self,
        backend: &mut B,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        if backend.check_currentness().is_err() {
            return self.poison(Gfx942CompletionErrorV1::Currentness);
        }
        Ok(())
    }

    fn require_ready(&self) -> Result<(), Gfx942CompletionErrorV1> {
        if self.phase == CompletionOwnerPhaseV1::Ready {
            Ok(())
        } else {
            Err(Gfx942CompletionErrorV1::Poisoned)
        }
    }

    fn poison<T>(&mut self, error: Gfx942CompletionErrorV1) -> Result<T, Gfx942CompletionErrorV1> {
        self.phase = CompletionOwnerPhaseV1::Poisoned;
        Err(error)
    }
}

pub(super) fn initialize_pending_completion_signal_arena(
    bytes: &mut [u8],
) -> Result<(), Gfx942CompletionErrorV1> {
    if bytes.len() != COMPLETION_SIGNAL_ARENA_BYTES_V1
        || !(bytes.as_ptr() as usize).is_multiple_of(AMD_SIGNAL_ALIGNMENT_V1)
    {
        return Err(Gfx942CompletionErrorV1::Initialization);
    }
    for slot in bytes.chunks_exact_mut(AMD_SIGNAL_BYTES_V1) {
        let signal = slot.as_mut_ptr().cast::<AmdBusyCompletionSignalV1>();
        // SAFETY: the exclusively borrowed arena is 64-byte aligned, consists
        // of exact 64-byte slots, and is initialized before GPU mapping. Each
        // write starts one non-overlapping signal object's lifetime.
        unsafe { signal.write(AmdBusyCompletionSignalV1::new_pending()) };
    }
    Ok(())
}

fn validate_packet_count<const N: usize>() -> Result<(), Gfx942CompletionErrorV1> {
    if N == 0 {
        return Err(Gfx942CompletionErrorV1::ZeroPacketCount);
    }
    if N > COMPLETION_SIGNAL_CAPACITY_V1 {
        return Err(Gfx942CompletionErrorV1::PacketCountExceedsMaximum {
            requested: N,
            maximum: COMPLETION_SIGNAL_CAPACITY_V1,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_aql::{
        AMD_SIGNAL_VALUE_COMPLETE_V1, AMD_SIGNAL_VALUE_PENDING_V1,
        AqlPacketBatchPublicationTargetV1, classify_acquired_completion_value_v1,
        encode_pending_completion_signal_bytes_v1,
    };
    use fe2o3_runtime_model::{
        AllocationGenerationV1, AllocationIdV1, DeviceGenerationV1, DeviceKeyV1, MappingIdV1,
        MemoryAllocationKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1, VmIdV1,
        VmKeyV1,
    };
    use sha2::{Digest, Sha256};

    #[repr(C, align(64))]
    struct AlignedArena([u8; COMPLETION_SIGNAL_ARENA_BYTES_V1]);

    struct MockBackend {
        values: [i64; COMPLETION_SIGNAL_CAPACITY_V1],
        currentness_calls: usize,
        fail_currentness_at: Option<usize>,
        observe_calls: usize,
        fail_observe_at: Option<usize>,
        reset_calls: usize,
        fail_reset_at: Option<usize>,
    }

    #[derive(Default)]
    struct PacketCapture {
        signals: Vec<u64>,
        headers: usize,
    }

    impl AqlPacketBatchPublicationTargetV1 for PacketCapture {
        type Error = ();

        fn write_unpublished(
            &mut self,
            _batch_index: u32,
            packet: &AqlKernelDispatchPacketV1,
        ) -> Result<(), Self::Error> {
            self.signals.push(packet.completion_signal());
            Ok(())
        }

        fn publish_release_header(
            &mut self,
            _batch_index: u32,
            _header: u16,
        ) -> Result<(), Self::Error> {
            self.headers += 1;
            Ok(())
        }
    }

    impl MockBackend {
        fn pending() -> Self {
            Self {
                values: [AMD_SIGNAL_VALUE_PENDING_V1; COMPLETION_SIGNAL_CAPACITY_V1],
                currentness_calls: 0,
                fail_currentness_at: None,
                observe_calls: 0,
                fail_observe_at: None,
                reset_calls: 0,
                fail_reset_at: None,
            }
        }
    }

    impl NativeCompletionSignalBackendV1 for MockBackend {
        fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
            self.currentness_calls += 1;
            if self.fail_currentness_at == Some(self.currentness_calls) {
                Err(Gfx942CompletionErrorV1::Currentness)
            } else {
                Ok(())
            }
        }

        fn observe_acquire(
            &mut self,
            slot_index: u32,
        ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
            self.observe_calls += 1;
            if self.fail_observe_at == Some(self.observe_calls) {
                return Err(Gfx942CompletionErrorV1::Observation);
            }
            Ok(classify_acquired_completion_value_v1(
                self.values[slot_index as usize],
            ))
        }

        fn reset_pending_release(
            &mut self,
            slot_index: u32,
        ) -> Result<(), Gfx942CompletionErrorV1> {
            self.reset_calls += 1;
            if self.fail_reset_at == Some(self.reset_calls) {
                return Err(Gfx942CompletionErrorV1::Recycle);
            }
            self.values[slot_index as usize] = AMD_SIGNAL_VALUE_PENDING_V1;
            Ok(())
        }
    }

    fn vm(device_generation: u64, vm_id: u64) -> VmKeyV1 {
        VmKeyV1 {
            device: DeviceKeyV1 {
                physical: PhysicalDeviceIdV1(7),
                generation: DeviceGenerationV1(device_generation),
            },
            id: VmIdV1(vm_id),
        }
    }

    fn mapping(vm: VmKeyV1, id: u64, generation: u64) -> MemoryMappingKeyV1 {
        MemoryMappingKeyV1 {
            allocation: MemoryAllocationKeyV1 {
                vm,
                id: AllocationIdV1(id),
                generation: AllocationGenerationV1(generation),
            },
            id: MappingIdV1(id),
        }
    }

    fn queue() -> QueueKeyV1 {
        QueueKeyV1 {
            vm: vm(3, 11),
            id: QueueInstanceIdV1(19),
            generation: QueueGenerationV1(5),
        }
    }

    fn owner() -> CompletionSignalArenaOwnerV1 {
        CompletionSignalArenaOwnerV1 {
            queue: queue(),
            signal_mapping: mapping(queue().vm, 23, 7),
            gpu_base: 0x20_0000,
            next_batch_id: 1,
            slots: [CompletionSlotRecordV1 {
                generation: 1,
                phase: CompletionSlotPhaseV1::Available,
            }; COMPLETION_SIGNAL_CAPACITY_V1],
            phase: CompletionOwnerPhaseV1::Ready,
        }
    }

    fn template(index: u64) -> CompletionPacketTemplateV1 {
        CompletionPacketTemplateV1::new(
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            0,
            ObservedGpuAddressV1::new(0x40_0000).unwrap(),
            ObservedGpuAddressV1::new(0x50_0000 + index * 16).unwrap(),
            16,
            CompletionDispatchGenerationBindingV1::new(
                queue(),
                mapping(queue().vm, 31 + index * 2, 2),
                mapping(queue().vm, 32 + index * 2, 4),
            ),
        )
    }

    fn publish<const N: usize>(
        owner: &mut CompletionSignalArenaOwnerV1,
        templates: [CompletionPacketTemplateV1; N],
    ) -> Gfx942CompletionBatchV1<N> {
        let bound = owner.bind_batch(templates).unwrap();
        let (_, retention) = bound.into_parts();
        owner.validate_bound(&retention).unwrap();
        owner.mark_published(retention, 99).unwrap()
    }

    #[test]
    fn exact_arena_initialization_matches_every_frozen_signal_image() {
        let mut arena = AlignedArena([0xaa; COMPLETION_SIGNAL_ARENA_BYTES_V1]);
        initialize_pending_completion_signal_arena(&mut arena.0).unwrap();
        for signal in arena.0.chunks_exact(AMD_SIGNAL_BYTES_V1) {
            assert_eq!(signal, encode_pending_completion_signal_bytes_v1());
        }
        assert_eq!(
            initialize_pending_completion_signal_arena(&mut arena.0[..AMD_SIGNAL_BYTES_V1]),
            Err(Gfx942CompletionErrorV1::Initialization)
        );
        let mut misaligned = [0_u8; COMPLETION_SIGNAL_ARENA_BYTES_V1 + 1];
        assert_eq!(
            initialize_pending_completion_signal_arena(&mut misaligned[1..]),
            Err(Gfx942CompletionErrorV1::Initialization)
        );
    }

    #[test]
    fn completion_manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_AQL_COMPLETION_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(rendered, GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1);
    }

    #[test]
    fn boundary_batches_bind_distinct_wrap_free_signal_slots() {
        for count in [1_usize, 2, 4, 16, 256] {
            let mut owner = owner();
            let templates: Vec<_> = (0..count).map(|index| template(index as u64)).collect();
            match count {
                1 => assert!(owner.bind_batch([templates[0]]).is_ok()),
                2 => assert!(owner.bind_batch([templates[0], templates[1]]).is_ok()),
                4 => assert!(
                    owner
                        .bind_batch([templates[0], templates[1], templates[2], templates[3]])
                        .is_ok()
                ),
                16 => {
                    let values: [CompletionPacketTemplateV1; 16] = templates.try_into().unwrap();
                    assert!(owner.bind_batch(values).is_ok());
                }
                256 => {
                    let values: [CompletionPacketTemplateV1; 256] = templates.try_into().unwrap();
                    assert!(owner.bind_batch(values).is_ok());
                }
                _ => unreachable!(),
            }
        }
        let mut zero = owner();
        assert!(matches!(
            zero.bind_batch([]),
            Err(Gfx942CompletionErrorV1::ZeroPacketCount)
        ));
        let mut over = owner();
        let over_values: [CompletionPacketTemplateV1; 257] =
            core::array::from_fn(|index| template(index as u64));
        assert!(matches!(
            over.bind_batch(over_values),
            Err(Gfx942CompletionErrorV1::PacketCountExceedsMaximum { .. })
        ));

        let mut exact = owner();
        let bound = exact
            .bind_batch([template(0), template(1), template(2), template(3)])
            .unwrap();
        let (packets, _) = bound.into_parts();
        let mut capture = PacketCapture::default();
        packets.publish_with(&mut capture).unwrap();
        assert_eq!(
            capture.signals,
            vec![0x20_0000, 0x20_0040, 0x20_0080, 0x20_00c0]
        );
        assert_eq!(capture.headers, 4);
    }

    #[test]
    fn binding_rejects_wrong_queue_vm_and_packet_without_mutation() {
        let mut owner = owner();
        let mut wrong_queue = template(0);
        wrong_queue.generations.queue.generation = QueueGenerationV1(6);
        assert!(matches!(
            owner.bind_batch([wrong_queue]),
            Err(Gfx942CompletionErrorV1::WrongQueueGeneration)
        ));
        let mut wrong_vm = template(0);
        wrong_vm.generations.data.allocation.vm = vm(3, 12);
        assert!(matches!(
            owner.bind_batch([wrong_vm]),
            Err(Gfx942CompletionErrorV1::WrongVmGeneration)
        ));
        let mut invalid_packet = template(0);
        invalid_packet.kernarg_alignment = 3;
        assert!(matches!(
            owner.bind_batch([invalid_packet]),
            Err(Gfx942CompletionErrorV1::PacketBinding(_))
        ));
        let mut invalid_second = template(1);
        invalid_second.kernarg_alignment = 3;
        assert!(matches!(
            owner.bind_batch([template(0), invalid_second]),
            Err(Gfx942CompletionErrorV1::PacketBinding(_))
        ));
        assert!(owner.bind_batch([template(0), template(1)]).is_ok());
    }

    #[test]
    fn pending_ready_and_recycle_are_exact_for_unique_signals() {
        let mut owner = owner();
        let batch = publish(
            &mut owner,
            [template(0), template(1), template(2), template(3)],
        );
        let mut backend = MockBackend::pending();
        let batch = match owner.observe_once(batch, &mut backend).unwrap() {
            Gfx942CompletionPollV1::Pending(batch) => batch,
            Gfx942CompletionPollV1::Ready(_) => panic!("pending batch reported ready"),
        };
        backend.values[..4].fill(AMD_SIGNAL_VALUE_COMPLETE_V1);
        let completed = match owner.observe_once(batch, &mut backend).unwrap() {
            Gfx942CompletionPollV1::Ready(completed) => completed,
            Gfx942CompletionPollV1::Pending(_) => panic!("completed batch reported pending"),
        };
        let observation = owner.recycle(completed, &mut backend).unwrap();
        assert_eq!(observation.packet_count(), 4);
        assert_eq!(backend.values[..4], [AMD_SIGNAL_VALUE_PENDING_V1; 4]);
        assert!(owner.ensure_releasable().is_ok());
        assert!(owner.bind_batch([template(4); 4]).is_ok());
    }

    #[test]
    fn fault_timeout_and_ambiguous_observation_poison() {
        let mut fault_owner = owner();
        let fault_batch = publish(&mut fault_owner, [template(0), template(1)]);
        let mut fault_backend = MockBackend::pending();
        fault_backend.values[1] = -7;
        assert!(matches!(
            fault_owner.observe_once(fault_batch, &mut fault_backend),
            Err(Gfx942CompletionErrorV1::Fault { slot: 1, value: -7 })
        ));
        assert_eq!(
            fault_owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );

        let mut timeout_owner = owner();
        let timeout_batch = publish(&mut timeout_owner, [template(0)]);
        assert!(matches!(
            timeout_owner.wait_bounded(timeout_batch, 3, &mut MockBackend::pending()),
            Err(Gfx942CompletionErrorV1::Timeout { polls: 3 })
        ));

        let mut observe_owner = owner();
        let observe_batch = publish(&mut observe_owner, [template(0)]);
        let mut observe_backend = MockBackend::pending();
        observe_backend.fail_observe_at = Some(1);
        assert!(matches!(
            observe_owner.observe_once(observe_batch, &mut observe_backend),
            Err(Gfx942CompletionErrorV1::Observation)
        ));
    }

    #[test]
    fn every_currentness_and_recycle_boundary_fails_closed() {
        for fail_at in [1_usize, 2] {
            let mut owner = owner();
            let batch = publish(&mut owner, [template(0)]);
            let mut backend = MockBackend::pending();
            backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
            backend.fail_currentness_at = Some(fail_at);
            assert!(matches!(
                owner.observe_once(batch, &mut backend),
                Err(Gfx942CompletionErrorV1::Currentness)
            ));
            assert_eq!(
                owner.ensure_releasable(),
                Err(Gfx942CompletionErrorV1::Poisoned)
            );
        }

        for fail_reset_at in 1..=4 {
            let mut owner = owner();
            let batch = publish(
                &mut owner,
                [template(0), template(1), template(2), template(3)],
            );
            let mut backend = MockBackend::pending();
            backend.values[..4].fill(AMD_SIGNAL_VALUE_COMPLETE_V1);
            let completed = owner.wait_bounded(batch, 1, &mut backend).unwrap();
            backend.fail_reset_at = Some(fail_reset_at);
            assert_eq!(
                owner.recycle(completed, &mut backend),
                Err(Gfx942CompletionErrorV1::Recycle)
            );
            assert_eq!(
                owner.ensure_releasable(),
                Err(Gfx942CompletionErrorV1::Poisoned)
            );
        }

        for fail_currentness_at in [3_usize, 4] {
            let mut owner = owner();
            let batch = publish(&mut owner, [template(0)]);
            let mut backend = MockBackend::pending();
            backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
            let completed = owner.wait_bounded(batch, 1, &mut backend).unwrap();
            backend.fail_currentness_at = Some(fail_currentness_at);
            assert_eq!(
                owner.recycle(completed, &mut backend),
                Err(Gfx942CompletionErrorV1::Currentness)
            );
            assert_eq!(
                owner.ensure_releasable(),
                Err(Gfx942CompletionErrorV1::Poisoned)
            );
        }
    }

    #[test]
    fn observation_failure_at_every_batch_slot_is_terminal() {
        for fail_observe_at in 1..=4 {
            let mut owner = owner();
            let batch = publish(
                &mut owner,
                [template(0), template(1), template(2), template(3)],
            );
            let mut backend = MockBackend::pending();
            backend.fail_observe_at = Some(fail_observe_at);
            assert!(matches!(
                owner.observe_once(batch, &mut backend),
                Err(Gfx942CompletionErrorV1::Observation)
            ));
            assert_eq!(
                owner.ensure_releasable(),
                Err(Gfx942CompletionErrorV1::Poisoned)
            );
        }
    }

    #[test]
    fn stale_generation_and_live_batches_prevent_release() {
        let mut owner = owner();
        let bound = owner.bind_batch([template(0)]).unwrap();
        let (_, mut retention) = bound.into_parts();
        assert_eq!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::BatchStillRetained)
        );
        retention.slots[0].generation += 1;
        assert_eq!(
            owner.validate_bound(&retention),
            Err(Gfx942CompletionErrorV1::StaleBatchGeneration)
        );
    }

    #[test]
    fn capacity_and_identity_exhaustion_are_preflighted() {
        let mut full = owner();
        let all: [CompletionPacketTemplateV1; 256] =
            core::array::from_fn(|index| template(index as u64));
        assert!(full.bind_batch(all).is_ok());
        assert!(matches!(
            full.bind_batch([template(300)]),
            Err(Gfx942CompletionErrorV1::InsufficientSignals)
        ));

        let mut batch_ids = owner();
        batch_ids.next_batch_id = u64::MAX;
        assert!(matches!(
            batch_ids.bind_batch([template(0)]),
            Err(Gfx942CompletionErrorV1::BatchIdentityExhausted)
        ));
        assert!(batch_ids.ensure_releasable().is_ok());

        let mut generations = owner();
        generations.slots[0].generation = u64::MAX;
        let batch = publish(&mut generations, [template(0)]);
        let mut backend = MockBackend::pending();
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        let completed = generations.wait_bounded(batch, 1, &mut backend).unwrap();
        assert_eq!(
            generations.recycle(completed, &mut backend),
            Err(Gfx942CompletionErrorV1::SignalGenerationExhausted)
        );
        assert_eq!(backend.reset_calls, 0);
        assert_eq!(
            generations.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );
    }

    #[test]
    fn oversized_poll_bound_is_terminal_before_observation() {
        let mut owner = owner();
        let batch = publish(&mut owner, [template(0)]);
        let mut backend = MockBackend::pending();
        assert!(matches!(
            owner.wait_bounded(batch, MAX_COMPLETION_POLL_ATTEMPTS_V1 + 1, &mut backend),
            Err(Gfx942CompletionErrorV1::InvalidPollBound {
                requested,
                maximum: MAX_COMPLETION_POLL_ATTEMPTS_V1,
            }) if requested == MAX_COMPLETION_POLL_ATTEMPTS_V1 + 1
        ));
        assert_eq!(backend.observe_calls, 0);
        assert_eq!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );
    }
}

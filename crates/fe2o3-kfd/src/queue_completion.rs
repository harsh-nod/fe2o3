//! Private, bounded completion authority for the retained compute-AQL queue.
//!
//! Every fixed-batch packet and the isolated barrier probe receive distinct
//! ROCr user signals from one coherent GTT arena. Numeric addresses remain
//! inside this module. Fixed-batch state retains exact dispatch generations;
//! the barrier probe retains only its exact queue and signal generations until
//! completion has been observed and explicitly recycled.

use core::fmt;
use std::time::Instant;

use fe2o3_aql::{
    AMD_SIGNAL_ALIGNMENT_V1, AMD_SIGNAL_BYTES_V1, AQL_MAX_FIXED_BATCH_PACKETS_V2,
    AmdBusyCompletionSignalV1, AqlBarrierAndPacketErrorV1, AqlBarrierAndPacketV1,
    AqlCompletionObservationV1, AqlDispatchGeometryV1, AqlDispatchOrderingV1,
    AqlDispatchPacketError, AqlKernelDispatchPacketV1, AqlPreparedKernelDispatchBatchErrorV1,
    AqlPreparedKernelDispatchBatchV2, AqlPreparedKernelDispatchV1, ObservedGpuAddressV1,
};
use fe2o3_runtime_model::{MemoryMappingKeyV1, QueueKeyV1};

use crate::shared_memory::SharedGttMappedResourceFactsV1;
use crate::wait::MonotonicWaitV1;

pub(crate) const COMPLETION_SIGNAL_CAPACITY_V1: usize = AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize;
pub(crate) const COMPLETION_SIGNAL_ARENA_BYTES_V1: usize =
    COMPLETION_SIGNAL_CAPACITY_V1 * AMD_SIGNAL_BYTES_V1;
pub(super) const MAX_COMPLETION_POLL_ATTEMPTS_V1: u32 = 1_000_000;

/// Canonical claim boundary for the private completion-signal slice.
pub const GFX942_AQL_COMPLETION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-aql-completion-r8-v1\n",
    "aql_dispatch_schema_sha256=82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0\n",
    "aql_barrier_and_schema_sha256=bdca900cd5c6eaccbddfc5a854e956382a08ce87bec4ccd5284baacf932cdfb5\n",
    "aql_fixed_batch_schema_sha256=a3c74fe4aa26a62772253de267812f2fb1626247685d8c4e8ed8bbb2a5a9e34a\n",
    "arena=one-host-visible-coherent-gtt-allocation,524288-bytes,8192-distinct-64-byte-aligned-user-signals\n",
    "batch=1-through-8192,heap-owned-fixed-cardinality-state,one-unique-signal-per-packet,no-aggregate-alias\n",
    "initialization=typed-amd-busy-signal-construction,kind-user-1,value-pending-1,event-fields-zero,before-gpu-map\n",
    "fixed-batch-binding=crate-private-packet-construction,per-packet-independent-or-wait-for-prior-ordering-retained,no-public-signal-address,exact-queue-vm-signal-code-kernarg-and-nonzero-dispatch-generations-retained,actual-resource-lifetimes-owned-by-private-c5-queue-owner\n",
    "observation=monotonic-deadline-or-legacy-bounded-poll,short-spin-then-yield-and-bounded-exponential-sleep,one-pre-post-currentness-envelope-around-one-exact-retained-signal-set-of-atomic-i64-acquire-loads,same-scan-redacted-packet-completed-pending-and-first-pending-index-progress,all-retained-signals-zero-before-ready,unexpected-value-is-fault,timeout-retains-linear-operation-privately-until-addressless-counter-first-retained-packet-first-retained-signal-exception-currentness-snapshot\n",
    "recycle=fixed-batch-only-after-exact-all-signal-completion-or-barrier-probe-only-after-exact-one-signal-completion,atomic-i64-release-reset-to-pending,checked-slot-generation-increment\n",
    "barrier-probe=isolated-owner-phase,exact-one-slot,queue-and-signal-generations-only,no-code-kernarg-or-dispatch-generation,bound-published-completed-recycled-linear-custody,zero-dependency-system-scope-header-0x1403\n",
    "failure=currentness-native-observation-unexpected-value-timeout-invalid-poll-bound-generation-exhaustion-or-reset-ambiguity-poisons-owner-and-queue;timeout-snapshot-precedes-poison-and-grants-no-native-authority;teardown-required\n",
    "release=queue-destroy-first,only-when-every-batch-was-completed-and-recycled,explicit-unmap-and-free,no-drop-native-effects\n",
    "proof=host-state-machine-and-mock-fault-tests-only,cpu-gpu-atomic-coherence-device-write-visibility-firmware-signal-and-quiescence-refinement-contracted\n",
    "excluded=public-safe-launch,resource-lifetime-mint,copy,alias-proof,hardware-execution,ioctl-validation\n",
);

/// SHA-256 of [`GFX942_AQL_COMPLETION_MANIFEST_V1`].
pub const GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1: &str =
    "4b7e1090eccbae41ea09ce7d5147470eb665ee295cb0f4526f5584225c86369a";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionOwnerPhaseV1 {
    Ready,
    ProbeActive,
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

fn allocate_completion_slot_records_v1()
-> Result<Box<[CompletionSlotRecordV1; COMPLETION_SIGNAL_CAPACITY_V1]>, Gfx942CompletionErrorV1> {
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(COMPLETION_SIGNAL_CAPACITY_V1)
        .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("completion state allocation"))?;
    slots.resize(
        COMPLETION_SIGNAL_CAPACITY_V1,
        CompletionSlotRecordV1 {
            generation: 1,
            phase: CompletionSlotPhaseV1::Available,
        },
    );
    slots
        .into_boxed_slice()
        .try_into()
        .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("completion state cardinality"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompletionDispatchGenerationBindingV1 {
    queue: QueueKeyV1,
    code: MemoryMappingKeyV1,
    kernarg: MemoryMappingKeyV1,
    dispatch_generation: u64,
}

impl CompletionDispatchGenerationBindingV1 {
    #[allow(dead_code)]
    pub(crate) const fn new(
        queue: QueueKeyV1,
        code: MemoryMappingKeyV1,
        kernarg: MemoryMappingKeyV1,
        dispatch_generation: u64,
    ) -> Self {
        Self {
            queue,
            code,
            kernarg,
            dispatch_generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompletionPacketTemplateV1 {
    geometry: AqlDispatchGeometryV1,
    ordering: AqlDispatchOrderingV1,
    private_segment_size: u32,
    group_segment_size: u32,
    kernel_object: ObservedGpuAddressV1,
    kernarg_address: ObservedGpuAddressV1,
    kernarg_alignment: u64,
    generations: CompletionDispatchGenerationBindingV1,
}

struct CompletionPacketTemplatesV1<const N: usize> {
    values: Box<[CompletionPacketTemplateV1; N]>,
}

impl<const N: usize> CompletionPacketTemplatesV1<N> {
    fn from_array(values: [CompletionPacketTemplateV1; N]) -> Self {
        Self {
            values: Box::new(values),
        }
    }

    #[cfg(test)]
    fn try_from_vec(values: Vec<CompletionPacketTemplateV1>) -> Result<Self, ()> {
        Ok(Self {
            values: values.into_boxed_slice().try_into().map_err(|_| ())?,
        })
    }
}

impl CompletionPacketTemplateV1 {
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) const fn new(
        geometry: AqlDispatchGeometryV1,
        ordering: AqlDispatchOrderingV1,
        private_segment_size: u32,
        group_segment_size: u32,
        kernel_object: ObservedGpuAddressV1,
        kernarg_address: ObservedGpuAddressV1,
        kernarg_alignment: u64,
        generations: CompletionDispatchGenerationBindingV1,
    ) -> Self {
        Self {
            geometry,
            ordering,
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
pub(super) struct BarrierProbeRetentionV1 {
    probe_id: u64,
    queue: QueueKeyV1,
    signal_mapping: MemoryMappingKeyV1,
    slot: CompletionSlotLeaseV1,
    packet_id: Option<u64>,
}

pub(super) struct BoundBarrierProbeV1 {
    packet: fe2o3_aql::AqlPreparedBarrierAndV1,
    retention: BarrierProbeRetentionV1,
}

impl BoundBarrierProbeV1 {
    pub(super) fn into_parts(
        self,
    ) -> (fe2o3_aql::AqlPreparedBarrierAndV1, BarrierProbeRetentionV1) {
        (self.packet, self.retention)
    }
}

/// Linear custody for one published zero-dependency BARRIER_AND probe.
#[must_use = "a published barrier probe must be observed or retained for teardown"]
pub struct Gfx942BarrierProbeV1 {
    retention: BarrierProbeRetentionV1,
}

impl fmt::Debug for Gfx942BarrierProbeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942BarrierProbeV1")
            .finish_non_exhaustive()
    }
}

impl Gfx942BarrierProbeV1 {
    pub(super) fn packet_and_signal_slot(&self) -> Result<(u64, u32), Gfx942CompletionErrorV1> {
        Ok((
            self.retention
                .packet_id
                .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?,
            self.retention.slot.index,
        ))
    }
}

/// Linear completion evidence for one barrier probe.
#[must_use = "the completed barrier signal must be explicitly recycled"]
pub struct Gfx942CompletedBarrierProbeV1 {
    retention: BarrierProbeRetentionV1,
}

impl fmt::Debug for Gfx942CompletedBarrierProbeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942CompletedBarrierProbeV1")
            .finish_non_exhaustive()
    }
}

impl Gfx942CompletedBarrierProbeV1 {
    pub(super) fn packet_and_signal_slot(&self) -> Result<(u64, u32), Gfx942CompletionErrorV1> {
        Ok((
            self.retention
                .packet_id
                .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?,
            self.retention.slot.index,
        ))
    }
}

/// Same-scan, addressless progress for one barrier probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942BarrierProbeProgressV1 {
    signal: Gfx942TimeoutSignalObservationV1,
}

impl Gfx942BarrierProbeProgressV1 {
    pub const fn packet_count(self) -> u16 {
        1
    }

    pub const fn signal(self) -> Gfx942TimeoutSignalObservationV1 {
        self.signal
    }
}

/// Linear result of one nonblocking barrier-signal observation.
#[derive(Debug)]
pub enum Gfx942BarrierProbePollV1 {
    Pending {
        probe: Gfx942BarrierProbeV1,
        progress: Gfx942BarrierProbeProgressV1,
    },
    Ready {
        completed: Gfx942CompletedBarrierProbeV1,
        progress: Gfx942BarrierProbeProgressV1,
    },
}

/// Evidence that one completed barrier signal was reset to pending.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942BarrierProbeRecycleObservationV1;

impl Gfx942BarrierProbeRecycleObservationV1 {
    pub const fn packet_count(self) -> u16 {
        1
    }
}

pub(super) enum Gfx942BarrierProbeWaitFailureV1 {
    Terminal(Gfx942CompletionErrorV1),
    Timeout {
        probe: Box<Gfx942BarrierProbeV1>,
        polls: u32,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct CompletionBatchRetentionV1<const N: usize> {
    batch_id: u64,
    queue: QueueKeyV1,
    signal_mapping: MemoryMappingKeyV1,
    slots: Box<[CompletionSlotLeaseV1; N]>,
    dispatches: Box<[CompletionDispatchGenerationBindingV1; N]>,
    last_packet_id: Option<u64>,
}

pub(super) struct BoundCompletionBatchV1<const N: usize> {
    packets: AqlPreparedKernelDispatchBatchV2<N>,
    retention: CompletionBatchRetentionV1<N>,
}

impl<const N: usize> BoundCompletionBatchV1<N> {
    pub(super) fn into_parts(
        self,
    ) -> (
        AqlPreparedKernelDispatchBatchV2<N>,
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

impl<const N: usize> Gfx942CompletionBatchV1<N> {
    pub(super) fn first_packet_and_signal_slot(
        &self,
    ) -> Result<(u64, u32), Gfx942CompletionErrorV1> {
        let packet_count =
            u64::try_from(N).map_err(|_| Gfx942CompletionErrorV1::PacketCountExceedsMaximum {
                requested: N,
                maximum: AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize,
            })?;
        let first_packet_id = self
            .retention
            .last_packet_id
            .and_then(|last| last.checked_add(1))
            .and_then(|next| next.checked_sub(packet_count))
            .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
        let first_signal_slot = self
            .retention
            .slots
            .first()
            .ok_or(Gfx942CompletionErrorV1::ZeroPacketCount)?
            .index;
        Ok((first_packet_id, first_signal_slot))
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

/// Redacted progress observed while scanning one exact completion batch.
///
/// Signal loads occur sequentially, not as one atomic snapshot. Counts record
/// what that scan observed, and the first pending index can already be stale by
/// the time this value is returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942CompletionProgressV1 {
    packet_count: u16,
    completed_count: u16,
    pending_count: u16,
    first_pending_batch_index: Option<u16>,
}

impl Gfx942CompletionProgressV1 {
    pub const fn packet_count(self) -> u16 {
        self.packet_count
    }

    pub const fn completed_count(self) -> u16 {
        self.completed_count
    }

    pub const fn pending_count(self) -> u16 {
        self.pending_count
    }

    /// Returns the earliest batch-local index observed pending in this scan.
    pub const fn first_pending_batch_index(self) -> Option<u16> {
        self.first_pending_batch_index
    }
}

/// Linear completion custody paired with the progress from the same signal scan.
#[derive(Debug)]
pub enum Gfx942CompletionPollWithProgressV1<const N: usize> {
    Pending {
        batch: Gfx942CompletionBatchV1<N>,
        progress: Gfx942CompletionProgressV1,
    },
    Ready {
        completed: Gfx942CompletedBatchV1<N>,
        progress: Gfx942CompletionProgressV1,
    },
}

/// Crate-private, move-only proof that one exact completed batch was observed
/// inside a successful currentness envelope. It may only be consumed by the
/// immediate recycle continuation; public callers never receive this proof.
#[must_use = "the current completion handoff must be recycled or retained as completed custody"]
#[derive(Debug)]
pub(super) struct CompletionCurrentnessHandoffV1<const N: usize> {
    completed: Gfx942CompletedBatchV1<N>,
}

pub(super) enum CompletionPollWithCurrentnessHandoffV1<const N: usize> {
    Pending {
        batch: Gfx942CompletionBatchV1<N>,
        progress: Gfx942CompletionProgressV1,
    },
    Ready {
        handoff: CompletionCurrentnessHandoffV1<N>,
        progress: Gfx942CompletionProgressV1,
    },
}

impl<const N: usize> CompletionCurrentnessHandoffV1<N> {
    pub(super) fn into_completed(self) -> Gfx942CompletedBatchV1<N> {
        self.completed
    }
}

/// Addressless completion-signal state retained in a terminal timeout snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942TimeoutSignalObservationV1 {
    /// The acquired value was the canonical pending value.
    Pending,
    /// The acquired value was the canonical completed value.
    Completed,
    /// The acquired value was neither pending nor completed.
    Fault(i64),
}

impl Gfx942TimeoutSignalObservationV1 {
    /// Returns the exact acquired signal value represented by this observation.
    pub const fn value(self) -> i64 {
        match self {
            Self::Pending => fe2o3_aql::AMD_SIGNAL_VALUE_PENDING_V1,
            Self::Completed => fe2o3_aql::AMD_SIGNAL_VALUE_COMPLETE_V1,
            Self::Fault(value) => value,
        }
    }
}

/// Currentness-enveloped, addressless execution state for retained queue work.
///
/// The fields are sequential observations, not one atomic device snapshot. The
/// packet and signal selected for inspection remain private queue-relative
/// ordinals. Timeout paths capture this before poison; the one-shot barrier
/// success path captures it after completion and before signal recycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942TimeoutExecutionObservationV1 {
    packet_count: u16,
    write_counter: u64,
    read_counter: u64,
    first_packet_header: u16,
    first_packet_setup: u16,
    first_signal_kind: i64,
    first_signal: Gfx942TimeoutSignalObservationV1,
    queue_exception_reason_mask: u64,
}

impl Gfx942TimeoutExecutionObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        packet_count: u16,
        write_counter: u64,
        read_counter: u64,
        first_packet_header: u16,
        first_packet_setup: u16,
        first_signal_kind: i64,
        first_signal: Gfx942TimeoutSignalObservationV1,
        queue_exception_reason_mask: u64,
    ) -> Self {
        Self {
            packet_count,
            write_counter,
            read_counter,
            first_packet_header,
            first_packet_setup,
            first_signal_kind,
            first_signal,
            queue_exception_reason_mask,
        }
    }

    /// Returns the exact retained packet count represented by the snapshot.
    pub const fn packet_count(self) -> u16 {
        self.packet_count
    }

    /// Returns the acquiring write-counter observation.
    pub const fn write_counter(self) -> u64 {
        self.write_counter
    }

    /// Returns the acquiring read-counter observation.
    pub const fn read_counter(self) -> u64 {
        self.read_counter
    }

    /// Returns the first retained packet's acquiring low 16-bit header observation.
    pub const fn first_packet_header(self) -> u16 {
        self.first_packet_header
    }

    /// Returns the first retained packet's acquiring high 16-bit setup observation.
    pub const fn first_packet_setup(self) -> u16 {
        self.first_packet_setup
    }

    /// Returns the first retained signal's immutable kind-word observation.
    pub const fn first_signal_kind(self) -> i64 {
        self.first_signal_kind
    }

    /// Returns the first retained signal's acquiring value classification.
    pub const fn first_signal(self) -> Gfx942TimeoutSignalObservationV1 {
        self.first_signal
    }

    /// Returns the admitted volatile CWSR queue-exception reason mask.
    ///
    /// Zero is a racy observation at capture time, not proof that no exception
    /// occurred before or after the snapshot.
    pub const fn queue_exception_reason_mask(self) -> u64 {
        self.queue_exception_reason_mask
    }

    /// Confirms that device, runtime, event, and CWSR bindings were checked
    /// before and after the sequential observations.
    pub const fn currentness_confirmed(self) -> bool {
        true
    }
}

#[derive(Debug)]
pub(super) enum Gfx942CompletionWaitFailureV1<const N: usize> {
    Terminal(Gfx942CompletionErrorV1),
    Timeout {
        batch: Box<Gfx942CompletionBatchV1<N>>,
        polls: u32,
    },
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
    PacketCountExceedsMaximum {
        requested: usize,
        maximum: usize,
    },
    InsufficientSignals,
    BatchIdentityExhausted,
    SignalGenerationExhausted,
    WrongQueueGeneration,
    WrongVmGeneration,
    StaleBatchGeneration,
    Poisoned,
    Initialization,
    PacketBinding(AqlDispatchPacketError),
    BarrierPacketBinding(AqlBarrierAndPacketErrorV1),
    BatchConstruction(AqlPreparedKernelDispatchBatchErrorV1),
    Currentness,
    Observation,
    Fault {
        slot: u32,
        value: i64,
    },
    InvalidPollBound {
        requested: u32,
        maximum: u32,
    },
    Timeout {
        /// Requested bounded poll count.
        polls: u32,
        /// Addressless state captured before terminal poison.
        observation: Box<Gfx942TimeoutExecutionObservationV1>,
    },
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
    fn observe_one_acquire_in_current_scope(
        &mut self,
        slot_index: u32,
    ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1>;
    fn observe_batch_acquire_in_current_scope(
        &mut self,
        slot_indices: &[u32],
    ) -> Result<Vec<AqlCompletionObservationV1>, Gfx942CompletionErrorV1>;

    fn observe_batch_acquire(
        &mut self,
        slot_indices: &[u32],
    ) -> Result<Vec<AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
        self.check_currentness()?;
        let observations = self.observe_batch_acquire_in_current_scope(slot_indices)?;
        self.check_currentness()?;
        Ok(observations)
    }

    fn observe_one_acquire(
        &mut self,
        slot_index: u32,
    ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
        self.check_currentness()?;
        let observation = self.observe_one_acquire_in_current_scope(slot_index)?;
        self.check_currentness()?;
        Ok(observation)
    }

    fn reset_pending_release(&mut self, slot_index: u32) -> Result<(), Gfx942CompletionErrorV1>;
}

pub(super) struct CompletionSignalArenaOwnerV1 {
    queue: QueueKeyV1,
    signal_mapping: MemoryMappingKeyV1,
    gpu_base: u64,
    next_batch_id: u64,
    slots: Box<[CompletionSlotRecordV1; COMPLETION_SIGNAL_CAPACITY_V1]>,
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
            slots: allocate_completion_slot_records_v1()?,
            phase: CompletionOwnerPhaseV1::Ready,
        })
    }

    #[cfg(test)]
    pub(super) fn for_persistent_compute_cancellation_test(queue: QueueKeyV1) -> Self {
        Self {
            queue,
            signal_mapping: MemoryMappingKeyV1 {
                allocation: fe2o3_runtime_model::MemoryAllocationKeyV1 {
                    vm: queue.vm,
                    id: fe2o3_runtime_model::AllocationIdV1(1),
                    generation: fe2o3_runtime_model::AllocationGenerationV1(1),
                },
                id: fe2o3_runtime_model::MappingIdV1(1),
            },
            gpu_base: AMD_SIGNAL_ALIGNMENT_V1 as u64,
            next_batch_id: 1,
            slots: allocate_completion_slot_records_v1()
                .expect("fixed completion test roster is allocatable"),
            phase: CompletionOwnerPhaseV1::Ready,
        }
    }

    pub(super) fn bind_barrier_probe(
        &mut self,
    ) -> Result<BoundBarrierProbeV1, Gfx942CompletionErrorV1> {
        self.require_ready()?;
        if self
            .slots
            .iter()
            .any(|record| record.phase != CompletionSlotPhaseV1::Available)
        {
            return Err(Gfx942CompletionErrorV1::BatchStillRetained);
        }
        let next_probe_id = self
            .next_batch_id
            .checked_add(1)
            .ok_or(Gfx942CompletionErrorV1::BatchIdentityExhausted)?;
        let slot = CompletionSlotLeaseV1 {
            index: 0,
            generation: self.slots[0].generation,
        };
        let signal = ObservedGpuAddressV1::new(self.gpu_base)
            .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("completion address"))?;
        let packet = AqlBarrierAndPacketV1::new_unpublished(signal)
            .map_err(Gfx942CompletionErrorV1::BarrierPacketBinding)?;
        self.slots[0].phase = CompletionSlotPhaseV1::Bound {
            batch_id: self.next_batch_id,
        };
        let retention = BarrierProbeRetentionV1 {
            probe_id: self.next_batch_id,
            queue: self.queue,
            signal_mapping: self.signal_mapping,
            slot,
            packet_id: None,
        };
        self.next_batch_id = next_probe_id;
        self.phase = CompletionOwnerPhaseV1::ProbeActive;
        Ok(BoundBarrierProbeV1 { packet, retention })
    }

    pub(super) fn cancel_bound_barrier_probe(
        &mut self,
        retention: BarrierProbeRetentionV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        self.validate_barrier_probe(
            &retention,
            None,
            CompletionSlotPhaseV1::Bound {
                batch_id: retention.probe_id,
            },
        )?;
        self.slots[retention.slot.index as usize].phase = CompletionSlotPhaseV1::Available;
        self.phase = CompletionOwnerPhaseV1::Ready;
        Ok(())
    }

    pub(super) fn mark_barrier_probe_published(
        &mut self,
        mut retention: BarrierProbeRetentionV1,
        packet_id: u64,
    ) -> Result<Gfx942BarrierProbeV1, Gfx942CompletionErrorV1> {
        self.validate_barrier_probe(
            &retention,
            None,
            CompletionSlotPhaseV1::Bound {
                batch_id: retention.probe_id,
            },
        )?;
        self.slots[retention.slot.index as usize].phase = CompletionSlotPhaseV1::Published {
            batch_id: retention.probe_id,
        };
        retention.packet_id = Some(packet_id);
        Ok(Gfx942BarrierProbeV1 { retention })
    }

    pub(super) fn observe_barrier_probe_once<B: NativeCompletionSignalBackendV1>(
        &mut self,
        probe: Gfx942BarrierProbeV1,
        backend: &mut B,
    ) -> Result<Gfx942BarrierProbePollV1, Gfx942CompletionErrorV1> {
        self.validate_barrier_probe(
            &probe.retention,
            probe.retention.packet_id,
            CompletionSlotPhaseV1::Published {
                batch_id: probe.retention.probe_id,
            },
        )?;
        let observations = match backend.observe_batch_acquire(&[probe.retention.slot.index]) {
            Ok(observations) if observations.len() == 1 => observations,
            Ok(_) | Err(Gfx942CompletionErrorV1::Observation) => {
                return self.poison(Gfx942CompletionErrorV1::Observation);
            }
            Err(Gfx942CompletionErrorV1::Currentness) => {
                return self.poison(Gfx942CompletionErrorV1::Currentness);
            }
            Err(_) => return self.poison(Gfx942CompletionErrorV1::Observation),
        };
        match observations[0] {
            AqlCompletionObservationV1::Pending => Ok(Gfx942BarrierProbePollV1::Pending {
                probe,
                progress: Gfx942BarrierProbeProgressV1 {
                    signal: Gfx942TimeoutSignalObservationV1::Pending,
                },
            }),
            AqlCompletionObservationV1::Completed => {
                self.slots[probe.retention.slot.index as usize].phase =
                    CompletionSlotPhaseV1::Completed {
                        batch_id: probe.retention.probe_id,
                    };
                Ok(Gfx942BarrierProbePollV1::Ready {
                    completed: Gfx942CompletedBarrierProbeV1 {
                        retention: probe.retention,
                    },
                    progress: Gfx942BarrierProbeProgressV1 {
                        signal: Gfx942TimeoutSignalObservationV1::Completed,
                    },
                })
            }
            AqlCompletionObservationV1::Unexpected(value) => {
                self.poison(Gfx942CompletionErrorV1::Fault {
                    slot: probe.retention.slot.index,
                    value,
                })
            }
        }
    }

    pub(super) fn wait_barrier_probe_bounded<B: NativeCompletionSignalBackendV1>(
        &mut self,
        mut probe: Gfx942BarrierProbeV1,
        polls: u32,
        backend: &mut B,
    ) -> Result<Gfx942CompletedBarrierProbeV1, Gfx942BarrierProbeWaitFailureV1> {
        if polls > MAX_COMPLETION_POLL_ATTEMPTS_V1 {
            return self
                .poison(Gfx942CompletionErrorV1::InvalidPollBound {
                    requested: polls,
                    maximum: MAX_COMPLETION_POLL_ATTEMPTS_V1,
                })
                .map_err(Gfx942BarrierProbeWaitFailureV1::Terminal);
        }
        self.validate_barrier_probe(
            &probe.retention,
            probe.retention.packet_id,
            CompletionSlotPhaseV1::Published {
                batch_id: probe.retention.probe_id,
            },
        )
        .map_err(Gfx942BarrierProbeWaitFailureV1::Terminal)?;
        let mut wait = MonotonicWaitV1::without_deadline();
        for poll in 0..polls {
            match self
                .observe_barrier_probe_once(probe, backend)
                .map_err(Gfx942BarrierProbeWaitFailureV1::Terminal)?
            {
                Gfx942BarrierProbePollV1::Pending {
                    probe: pending,
                    progress,
                } => {
                    debug_assert_eq!(progress.packet_count(), 1);
                    debug_assert_eq!(progress.signal(), Gfx942TimeoutSignalObservationV1::Pending);
                    probe = pending;
                    if poll + 1 < polls {
                        wait.pause();
                    }
                }
                Gfx942BarrierProbePollV1::Ready {
                    completed,
                    progress,
                } => {
                    debug_assert_eq!(progress.packet_count(), 1);
                    debug_assert_eq!(
                        progress.signal(),
                        Gfx942TimeoutSignalObservationV1::Completed
                    );
                    return Ok(completed);
                }
            }
        }
        Err(Gfx942BarrierProbeWaitFailureV1::Timeout {
            probe: Box::new(probe),
            polls,
        })
    }

    pub(super) fn recycle_barrier_probe<B: NativeCompletionSignalBackendV1>(
        &mut self,
        completed: Gfx942CompletedBarrierProbeV1,
        backend: &mut B,
    ) -> Result<Gfx942BarrierProbeRecycleObservationV1, Gfx942CompletionErrorV1> {
        self.validate_barrier_probe(
            &completed.retention,
            completed.retention.packet_id,
            CompletionSlotPhaseV1::Completed {
                batch_id: completed.retention.probe_id,
            },
        )?;
        let record = &self.slots[completed.retention.slot.index as usize];
        let Some(next_generation) = record.generation.checked_add(1) else {
            return self.poison(Gfx942CompletionErrorV1::SignalGenerationExhausted);
        };
        self.checked_currentness(backend)?;
        if backend
            .reset_pending_release(completed.retention.slot.index)
            .is_err()
        {
            return self.poison(Gfx942CompletionErrorV1::Recycle);
        }
        self.checked_currentness(backend)?;
        let record = &mut self.slots[completed.retention.slot.index as usize];
        record.generation = next_generation;
        record.phase = CompletionSlotPhaseV1::Available;
        self.phase = CompletionOwnerPhaseV1::Ready;
        Ok(Gfx942BarrierProbeRecycleObservationV1)
    }

    pub(super) fn bind_batch<const N: usize>(
        &mut self,
        templates: [CompletionPacketTemplateV1; N],
    ) -> Result<BoundCompletionBatchV1<N>, Gfx942CompletionErrorV1> {
        self.bind_fixed_batch(CompletionPacketTemplatesV1::from_array(templates))
    }

    fn bind_fixed_batch<const N: usize>(
        &mut self,
        templates: CompletionPacketTemplatesV1<N>,
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
        for (template, slot_index) in templates.values.iter().zip(&available) {
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
                AqlKernelDispatchPacketV1::new_unpublished_with_ordering(
                    template.geometry,
                    template.private_segment_size,
                    template.group_segment_size,
                    template.kernel_object,
                    template.kernarg_address,
                    template.kernarg_alignment,
                    signal,
                    template.ordering,
                )
                .map_err(Gfx942CompletionErrorV1::PacketBinding)?,
            );
        }
        let packets: Box<[AqlPreparedKernelDispatchV1; N]> = prepared
            .into_boxed_slice()
            .try_into()
            .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("packet array conversion"))?;
        let packets = AqlPreparedKernelDispatchBatchV2::try_from_boxed_packets(packets)
            .map_err(Gfx942CompletionErrorV1::BatchConstruction)?;
        let slots: Box<[CompletionSlotLeaseV1; N]> = available
            .iter()
            .map(|index| CompletionSlotLeaseV1 {
                index: *index as u32,
                generation: self.slots[*index].generation,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("slot array conversion"))?;
        for slot in slots.iter() {
            self.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Bound {
                batch_id: self.next_batch_id,
            };
        }
        let dispatches: Box<[CompletionDispatchGenerationBindingV1; N]> = templates
            .values
            .iter()
            .map(|template| template.generations)
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .try_into()
            .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("dispatch array conversion"))?;
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
        for slot in retention.slots.iter() {
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
        for slot in retention.slots.iter() {
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
        match self.observe_once_with_progress(batch, backend)? {
            Gfx942CompletionPollWithProgressV1::Pending { batch, .. } => {
                Ok(Gfx942CompletionPollV1::Pending(batch))
            }
            Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => {
                Ok(Gfx942CompletionPollV1::Ready(completed))
            }
        }
    }

    pub(super) fn observe_once_with_progress<const N: usize, B: NativeCompletionSignalBackendV1>(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
        backend: &mut B,
    ) -> Result<Gfx942CompletionPollWithProgressV1<N>, Gfx942CompletionErrorV1> {
        self.observe_once_with_progress_retaining(batch, backend)
            .map_err(|(error, _batch)| error)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn observe_once_with_progress_retaining<
        const N: usize,
        B: NativeCompletionSignalBackendV1,
    >(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
        backend: &mut B,
    ) -> Result<
        Gfx942CompletionPollWithProgressV1<N>,
        (Gfx942CompletionErrorV1, Gfx942CompletionBatchV1<N>),
    > {
        match self.observe_once_with_progress_current_handoff_retaining(batch, backend) {
            Ok(CompletionPollWithCurrentnessHandoffV1::Pending { batch, progress }) => {
                Ok(Gfx942CompletionPollWithProgressV1::Pending { batch, progress })
            }
            Ok(CompletionPollWithCurrentnessHandoffV1::Ready { handoff, progress }) => {
                Ok(Gfx942CompletionPollWithProgressV1::Ready {
                    completed: handoff.into_completed(),
                    progress,
                })
            }
            Err(failure) => Err(failure),
        }
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn observe_once_with_progress_current_handoff_retaining<
        const N: usize,
        B: NativeCompletionSignalBackendV1,
    >(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
        backend: &mut B,
    ) -> Result<
        CompletionPollWithCurrentnessHandoffV1<N>,
        (Gfx942CompletionErrorV1, Gfx942CompletionBatchV1<N>),
    > {
        if let Err(error) = self.validate_observation_preflight(&batch) {
            return Err((error, batch));
        }
        let slot_indices: Vec<u32> = batch
            .retention
            .slots
            .iter()
            .map(|slot| slot.index)
            .collect();
        let observations = match backend.observe_batch_acquire(&slot_indices) {
            Ok(observations) => observations,
            Err(Gfx942CompletionErrorV1::Observation) => {
                self.phase = CompletionOwnerPhaseV1::Poisoned;
                return Err((Gfx942CompletionErrorV1::Observation, batch));
            }
            Err(Gfx942CompletionErrorV1::Currentness) => {
                self.phase = CompletionOwnerPhaseV1::Poisoned;
                return Err((Gfx942CompletionErrorV1::Currentness, batch));
            }
            Err(_) => {
                self.phase = CompletionOwnerPhaseV1::Poisoned;
                return Err((Gfx942CompletionErrorV1::Observation, batch));
            }
        };
        self.classify_completion_observations(batch, observations)
    }

    fn validate_observation_preflight<const N: usize>(
        &self,
        batch: &Gfx942CompletionBatchV1<N>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        self.require_ready()?;
        self.validate_published(&batch.retention)
    }

    #[allow(clippy::result_large_err)]
    fn classify_completion_observations<const N: usize, I>(
        &mut self,
        batch: Gfx942CompletionBatchV1<N>,
        observations: I,
    ) -> Result<
        CompletionPollWithCurrentnessHandoffV1<N>,
        (Gfx942CompletionErrorV1, Gfx942CompletionBatchV1<N>),
    >
    where
        I: IntoIterator<Item = AqlCompletionObservationV1>,
        I::IntoIter: ExactSizeIterator,
    {
        let mut observations = observations.into_iter();
        if observations.len() != N {
            self.phase = CompletionOwnerPhaseV1::Poisoned;
            return Err((Gfx942CompletionErrorV1::Observation, batch));
        }
        let mut completed_count = 0_u16;
        let mut pending_count = 0_u16;
        let mut first_pending_batch_index = None;
        for (batch_index, slot) in batch.retention.slots.iter().enumerate() {
            let observation = observations
                .next()
                .expect("exact completion observation cardinality checked");
            match observation {
                AqlCompletionObservationV1::Pending => {
                    pending_count += 1;
                    if first_pending_batch_index.is_none() {
                        first_pending_batch_index = Some(batch_index as u16);
                    }
                }
                AqlCompletionObservationV1::Completed => completed_count += 1,
                AqlCompletionObservationV1::Unexpected(value) => {
                    self.phase = CompletionOwnerPhaseV1::Poisoned;
                    return Err((
                        Gfx942CompletionErrorV1::Fault {
                            slot: slot.index,
                            value,
                        },
                        batch,
                    ));
                }
            }
        }
        debug_assert!(observations.next().is_none());
        let progress = Gfx942CompletionProgressV1 {
            packet_count: N as u16,
            completed_count,
            pending_count,
            first_pending_batch_index,
        };
        if pending_count != 0 {
            return Ok(CompletionPollWithCurrentnessHandoffV1::Pending { batch, progress });
        }
        for slot in batch.retention.slots.iter() {
            self.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Completed {
                batch_id: batch.retention.batch_id,
            };
        }
        Ok(CompletionPollWithCurrentnessHandoffV1::Ready {
            handoff: CompletionCurrentnessHandoffV1 {
                completed: Gfx942CompletedBatchV1 {
                    retention: batch.retention,
                },
            },
            progress,
        })
    }

    /// One-packet specialization used by persistent full-range compute. It
    /// preserves the exact generic completion semantics without constructing
    /// either a slot-index or observation `Vec` on the hot path.
    #[allow(clippy::result_large_err)]
    pub(super) fn observe_one_with_progress_current_handoff_retaining<
        B: NativeCompletionSignalBackendV1,
    >(
        &mut self,
        batch: Gfx942CompletionBatchV1<1>,
        backend: &mut B,
    ) -> Result<
        CompletionPollWithCurrentnessHandoffV1<1>,
        (Gfx942CompletionErrorV1, Gfx942CompletionBatchV1<1>),
    > {
        if let Err(error) = self.validate_observation_preflight(&batch) {
            return Err((error, batch));
        }
        let observation = match backend.observe_one_acquire(batch.retention.slots[0].index) {
            Ok(observation) => observation,
            Err(Gfx942CompletionErrorV1::Currentness) => {
                self.phase = CompletionOwnerPhaseV1::Poisoned;
                return Err((Gfx942CompletionErrorV1::Currentness, batch));
            }
            Err(_) => {
                self.phase = CompletionOwnerPhaseV1::Poisoned;
                return Err((Gfx942CompletionErrorV1::Observation, batch));
            }
        };
        self.classify_completion_observations(batch, core::iter::once(observation))
    }

    /// Recycles a batch whose exact completion and closing currentness check
    /// immediately precede this call in one private queue orchestration. The
    /// handoff replaces only recycle's duplicate opening currentness check.
    #[allow(clippy::result_large_err)]
    pub(super) fn recycle_current_handoff_retaining<
        const N: usize,
        B: NativeCompletionSignalBackendV1,
    >(
        &mut self,
        handoff: CompletionCurrentnessHandoffV1<N>,
        backend: &mut B,
    ) -> Result<
        Gfx942CompletionRecycleObservationV1,
        (Gfx942CompletionErrorV1, CompletionCurrentnessHandoffV1<N>),
    > {
        let CompletionCurrentnessHandoffV1 { completed } = handoff;
        if let Err(error) = self.require_ready() {
            return Err((error, CompletionCurrentnessHandoffV1 { completed }));
        }
        if let Err(error) = self.validate_completed(&completed.retention) {
            return Err((error, CompletionCurrentnessHandoffV1 { completed }));
        }
        if completed.retention.slots.iter().any(|slot| {
            self.slots[slot.index as usize]
                .generation
                .checked_add(1)
                .is_none()
        }) {
            self.phase = CompletionOwnerPhaseV1::Poisoned;
            return Err((
                Gfx942CompletionErrorV1::SignalGenerationExhausted,
                CompletionCurrentnessHandoffV1 { completed },
            ));
        }
        for slot in completed.retention.slots.iter() {
            if backend.reset_pending_release(slot.index).is_err() {
                self.phase = CompletionOwnerPhaseV1::Poisoned;
                return Err((
                    Gfx942CompletionErrorV1::Recycle,
                    CompletionCurrentnessHandoffV1 { completed },
                ));
            }
        }
        if let Err(error) = self.checked_currentness(backend) {
            // A successful reset cannot be authenticated after currentness is
            // lost. Retain Completed custody and poison rather than claiming
            // that the signal slot is safely reusable.
            return Err((error, CompletionCurrentnessHandoffV1 { completed }));
        }
        for slot in completed.retention.slots.iter() {
            let record = &mut self.slots[slot.index as usize];
            record.generation += 1;
            record.phase = CompletionSlotPhaseV1::Available;
        }
        Ok(Gfx942CompletionRecycleObservationV1 {
            packet_count: N as u16,
        })
    }

    pub(super) fn wait_bounded<const N: usize, B: NativeCompletionSignalBackendV1>(
        &mut self,
        mut batch: Gfx942CompletionBatchV1<N>,
        polls: u32,
        backend: &mut B,
    ) -> Result<Gfx942CompletedBatchV1<N>, Gfx942CompletionWaitFailureV1<N>> {
        if polls > MAX_COMPLETION_POLL_ATTEMPTS_V1 {
            return self
                .poison(Gfx942CompletionErrorV1::InvalidPollBound {
                    requested: polls,
                    maximum: MAX_COMPLETION_POLL_ATTEMPTS_V1,
                })
                .map_err(Gfx942CompletionWaitFailureV1::Terminal);
        }
        self.require_ready()
            .and_then(|()| self.validate_published(&batch.retention))
            .map_err(Gfx942CompletionWaitFailureV1::Terminal)?;
        if polls == 0 {
            return Err(Gfx942CompletionWaitFailureV1::Timeout {
                batch: Box::new(batch),
                polls,
            });
        }
        let mut wait = MonotonicWaitV1::without_deadline();
        for poll in 0..polls {
            match self
                .observe_once(batch, backend)
                .map_err(Gfx942CompletionWaitFailureV1::Terminal)?
            {
                Gfx942CompletionPollV1::Pending(pending) => {
                    batch = pending;
                    if poll + 1 < polls {
                        wait.pause();
                    }
                }
                Gfx942CompletionPollV1::Ready(ready) => return Ok(ready),
            }
        }
        Err(Gfx942CompletionWaitFailureV1::Timeout {
            batch: Box::new(batch),
            polls,
        })
    }

    pub(super) fn wait_until<const N: usize, B: NativeCompletionSignalBackendV1>(
        &mut self,
        mut batch: Gfx942CompletionBatchV1<N>,
        deadline: Instant,
        backend: &mut B,
    ) -> Result<Gfx942CompletedBatchV1<N>, Gfx942CompletionWaitFailureV1<N>> {
        self.require_ready()
            .and_then(|()| self.validate_published(&batch.retention))
            .map_err(Gfx942CompletionWaitFailureV1::Terminal)?;
        let mut wait = MonotonicWaitV1::until(deadline);
        let mut polls = 0_u32;
        loop {
            if wait.expired() || polls == u32::MAX {
                return Err(Gfx942CompletionWaitFailureV1::Timeout {
                    batch: Box::new(batch),
                    polls,
                });
            }
            polls += 1;
            match self
                .observe_once(batch, backend)
                .map_err(Gfx942CompletionWaitFailureV1::Terminal)?
            {
                Gfx942CompletionPollV1::Pending(pending) => batch = pending,
                Gfx942CompletionPollV1::Ready(ready) => return Ok(ready),
            }
            wait.pause();
        }
    }

    pub(super) fn recycle<const N: usize, B: NativeCompletionSignalBackendV1>(
        &mut self,
        completed: Gfx942CompletedBatchV1<N>,
        backend: &mut B,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942CompletionErrorV1> {
        self.recycle_retaining(completed, backend)
            .map_err(|(error, _completed)| error)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn recycle_retaining<const N: usize, B: NativeCompletionSignalBackendV1>(
        &mut self,
        completed: Gfx942CompletedBatchV1<N>,
        backend: &mut B,
    ) -> Result<
        Gfx942CompletionRecycleObservationV1,
        (Gfx942CompletionErrorV1, Gfx942CompletedBatchV1<N>),
    > {
        if let Err(error) = self.require_ready() {
            return Err((error, completed));
        }
        if let Err(error) = self.validate_completed(&completed.retention) {
            return Err((error, completed));
        }
        if completed.retention.slots.iter().any(|slot| {
            self.slots[slot.index as usize]
                .generation
                .checked_add(1)
                .is_none()
        }) {
            self.phase = CompletionOwnerPhaseV1::Poisoned;
            return Err((
                Gfx942CompletionErrorV1::SignalGenerationExhausted,
                completed,
            ));
        }
        if let Err(error) = self.checked_currentness(backend) {
            return Err((error, completed));
        }
        for slot in completed.retention.slots.iter() {
            if backend.reset_pending_release(slot.index).is_err() {
                self.phase = CompletionOwnerPhaseV1::Poisoned;
                return Err((Gfx942CompletionErrorV1::Recycle, completed));
            }
        }
        if let Err(error) = self.checked_currentness(backend) {
            return Err((error, completed));
        }
        for slot in completed.retention.slots.iter() {
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
        if binding.dispatch_generation == 0
            || binding.code.allocation.vm != self.queue.vm
            || binding.kernarg.allocation.vm != self.queue.vm
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
                || retention.dispatches[batch_index].dispatch_generation == 0
                || retention.dispatches[batch_index].code.allocation.vm != retention.queue.vm
                || retention.dispatches[batch_index].kernarg.allocation.vm != retention.queue.vm
                || retention.slots[..batch_index]
                    .iter()
                    .any(|prior| prior.index == slot.index)
            {
                return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
            }
        }
        Ok(())
    }

    fn validate_barrier_probe(
        &self,
        retention: &BarrierProbeRetentionV1,
        expected_packet_id: Option<u64>,
        expected_phase: CompletionSlotPhaseV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        self.require_probe_active()?;
        let Some(record) = self.slots.get(retention.slot.index as usize) else {
            return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
        };
        let requires_packet_id = matches!(
            expected_phase,
            CompletionSlotPhaseV1::Published { .. } | CompletionSlotPhaseV1::Completed { .. }
        );
        if retention.probe_id == 0
            || (requires_packet_id && retention.packet_id.is_none())
            || retention.queue != self.queue
            || retention.signal_mapping != self.signal_mapping
            || retention.slot.index != 0
            || retention.slot.generation != record.generation
            || retention.packet_id != expected_packet_id
            || record.phase != expected_phase
        {
            return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
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

    fn require_probe_active(&self) -> Result<(), Gfx942CompletionErrorV1> {
        if self.phase == CompletionOwnerPhaseV1::ProbeActive {
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
        AMD_SIGNAL_VALUE_COMPLETE_V1, AMD_SIGNAL_VALUE_PENDING_V1, AqlBarrierAndPacketV1,
        AqlBarrierAndPublicationTargetV1, AqlPacketBatchPublicationTargetV1,
        classify_acquired_completion_value_v1, encode_pending_completion_signal_bytes_v1,
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
        trace: Vec<&'static str>,
        currentness_calls: usize,
        fail_currentness_at: Option<usize>,
        observe_calls: usize,
        fail_observe_at: Option<usize>,
        extra_batch_observation: Option<AqlCompletionObservationV1>,
        reset_calls: usize,
        fail_reset_at: Option<usize>,
    }

    #[derive(Default)]
    struct PacketCapture {
        signals: Vec<u64>,
        headers: Vec<u16>,
    }

    #[derive(Default)]
    struct BarrierCapture {
        bytes: Option<[u8; 64]>,
        header: Option<u16>,
    }

    impl AqlBarrierAndPublicationTargetV1 for BarrierCapture {
        type Error = ();

        fn write_unpublished_barrier(
            &mut self,
            packet: &AqlBarrierAndPacketV1,
        ) -> Result<(), Self::Error> {
            self.bytes = Some(packet.encode_unpublished_le());
            Ok(())
        }

        fn publish_barrier_release_header(&mut self, header: u16) -> Result<(), Self::Error> {
            self.header = Some(header);
            Ok(())
        }
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
            header: u16,
        ) -> Result<(), Self::Error> {
            self.headers.push(header);
            Ok(())
        }
    }

    impl MockBackend {
        fn pending() -> Self {
            Self {
                values: [AMD_SIGNAL_VALUE_PENDING_V1; COMPLETION_SIGNAL_CAPACITY_V1],
                trace: Vec::new(),
                currentness_calls: 0,
                fail_currentness_at: None,
                observe_calls: 0,
                fail_observe_at: None,
                extra_batch_observation: None,
                reset_calls: 0,
                fail_reset_at: None,
            }
        }
    }

    impl NativeCompletionSignalBackendV1 for MockBackend {
        fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
            self.trace.push("currentness");
            self.currentness_calls += 1;
            if self.fail_currentness_at == Some(self.currentness_calls) {
                Err(Gfx942CompletionErrorV1::Currentness)
            } else {
                Ok(())
            }
        }

        fn observe_one_acquire_in_current_scope(
            &mut self,
            slot_index: u32,
        ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
            self.trace.push("acquire");
            self.observe_calls += 1;
            if self.fail_observe_at == Some(self.observe_calls) {
                return Err(Gfx942CompletionErrorV1::Observation);
            }
            Ok(classify_acquired_completion_value_v1(
                self.values[slot_index as usize],
            ))
        }

        fn observe_batch_acquire_in_current_scope(
            &mut self,
            slot_indices: &[u32],
        ) -> Result<Vec<AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
            self.trace.push("acquire");
            let mut observations = Vec::with_capacity(slot_indices.len());
            for &slot_index in slot_indices {
                self.observe_calls += 1;
                if self.fail_observe_at == Some(self.observe_calls) {
                    return Err(Gfx942CompletionErrorV1::Observation);
                }
                observations.push(classify_acquired_completion_value_v1(
                    self.values[slot_index as usize],
                ));
            }
            observations.extend(self.extra_batch_observation);
            Ok(observations)
        }

        fn reset_pending_release(
            &mut self,
            slot_index: u32,
        ) -> Result<(), Gfx942CompletionErrorV1> {
            self.trace.push("reset");
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
            slots: allocate_completion_slot_records_v1().unwrap(),
            phase: CompletionOwnerPhaseV1::Ready,
        }
    }

    #[test]
    fn completion_owner_retains_exact_heap_cardinality_with_bounded_inline_state() {
        let owner = owner();
        assert_eq!(owner.slots.len(), COMPLETION_SIGNAL_CAPACITY_V1);
        assert!(core::mem::size_of::<CompletionSignalArenaOwnerV1>() <= 128);
    }

    fn template(index: u64) -> CompletionPacketTemplateV1 {
        CompletionPacketTemplateV1::new(
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            AqlDispatchOrderingV1::WaitForPrior,
            0,
            0,
            ObservedGpuAddressV1::new(0x40_0000).unwrap(),
            ObservedGpuAddressV1::new(0x50_0000 + index * 16).unwrap(),
            16,
            CompletionDispatchGenerationBindingV1::new(
                queue(),
                mapping(queue().vm, 30, 1),
                mapping(queue().vm, 31 + index * 2, 2),
                4,
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

    fn publish_barrier(owner: &mut CompletionSignalArenaOwnerV1) -> Gfx942BarrierProbeV1 {
        let bound = owner.bind_barrier_probe().unwrap();
        let (_, retention) = bound.into_parts();
        owner.mark_barrier_probe_published(retention, 41).unwrap()
    }

    #[test]
    fn barrier_probe_binds_only_queue_and_signal_then_recycles() {
        let mut owner = owner();
        let bound = owner.bind_barrier_probe().unwrap();
        assert!(matches!(
            owner.bind_batch([template(0)]),
            Err(Gfx942CompletionErrorV1::Poisoned)
        ));
        let (packet, retention) = bound.into_parts();
        let mut capture = BarrierCapture::default();
        packet.publish_with(&mut capture).unwrap();
        let bytes = capture.bytes.unwrap();
        assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), 1);
        assert!(bytes[8..48].iter().all(|byte| *byte == 0));
        assert_eq!(
            u64::from_le_bytes(bytes[56..64].try_into().unwrap()),
            0x20_0000
        );
        assert_eq!(capture.header, Some(0x1403));

        let probe = owner.mark_barrier_probe_published(retention, 41).unwrap();
        let mut backend = MockBackend::pending();
        let probe = match owner
            .observe_barrier_probe_once(probe, &mut backend)
            .unwrap()
        {
            Gfx942BarrierProbePollV1::Pending { probe, progress } => {
                assert_eq!(progress.packet_count(), 1);
                assert_eq!(progress.signal(), Gfx942TimeoutSignalObservationV1::Pending);
                probe
            }
            Gfx942BarrierProbePollV1::Ready { .. } => panic!("pending probe reported ready"),
        };
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        let completed = match owner
            .observe_barrier_probe_once(probe, &mut backend)
            .unwrap()
        {
            Gfx942BarrierProbePollV1::Ready {
                completed,
                progress,
            } => {
                assert_eq!(
                    progress.signal(),
                    Gfx942TimeoutSignalObservationV1::Completed
                );
                completed
            }
            Gfx942BarrierProbePollV1::Pending { .. } => panic!("completed probe remained pending"),
        };
        assert_eq!(
            owner.recycle_barrier_probe(completed, &mut backend),
            Ok(Gfx942BarrierProbeRecycleObservationV1)
        );
        assert_eq!(backend.values[0], AMD_SIGNAL_VALUE_PENDING_V1);
        owner.ensure_releasable().unwrap();
        assert!(owner.bind_batch([template(0)]).is_ok());
    }

    #[test]
    fn barrier_probe_rejects_missing_packet_and_zero_identity() {
        let mut missing_packet = owner();
        let bound = missing_packet.bind_barrier_probe().unwrap();
        let (_, retention) = bound.into_parts();
        missing_packet.slots[0].phase = CompletionSlotPhaseV1::Published {
            batch_id: retention.probe_id,
        };
        let mut backend = MockBackend::pending();
        assert!(matches!(
            missing_packet
                .observe_barrier_probe_once(Gfx942BarrierProbeV1 { retention }, &mut backend),
            Err(Gfx942CompletionErrorV1::StaleBatchGeneration)
        ));

        let mut zero_identity = owner();
        let probe = publish_barrier(&mut zero_identity);
        let mut retention = probe.retention;
        zero_identity.slots[0].phase = CompletionSlotPhaseV1::Published { batch_id: 0 };
        retention.probe_id = 0;
        assert!(matches!(
            zero_identity
                .observe_barrier_probe_once(Gfx942BarrierProbeV1 { retention }, &mut backend),
            Err(Gfx942CompletionErrorV1::StaleBatchGeneration)
        ));
    }

    #[test]
    fn barrier_probe_generation_exhaustion_terminally_poisons_owner() {
        let mut owner = owner();
        let probe = publish_barrier(&mut owner);
        let mut backend = MockBackend::pending();
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        let Gfx942BarrierProbePollV1::Ready { mut completed, .. } = owner
            .observe_barrier_probe_once(probe, &mut backend)
            .unwrap()
        else {
            panic!("completed probe remained pending");
        };
        owner.slots[0].generation = u64::MAX;
        completed.retention.slot.generation = u64::MAX;
        assert_eq!(
            owner.recycle_barrier_probe(completed, &mut backend),
            Err(Gfx942CompletionErrorV1::SignalGenerationExhausted)
        );
        assert!(matches!(
            owner.bind_barrier_probe(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        ));
    }

    #[test]
    fn barrier_probe_cancel_restores_release_and_dispatch_admission() {
        let mut owner = owner();
        let bound = owner.bind_barrier_probe().unwrap();
        let (_, retention) = bound.into_parts();
        assert_eq!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );
        owner.cancel_bound_barrier_probe(retention).unwrap();
        owner.ensure_releasable().unwrap();
        assert!(owner.bind_batch([template(0)]).is_ok());
    }

    #[test]
    fn barrier_probe_timeout_retains_custody_until_outer_quarantine() {
        let mut owner = owner();
        let probe = publish_barrier(&mut owner);
        let mut backend = MockBackend::pending();
        let failure = owner
            .wait_barrier_probe_bounded(probe, 2, &mut backend)
            .unwrap_err();
        let Gfx942BarrierProbeWaitFailureV1::Timeout { probe, polls } = failure else {
            panic!("pending probe did not time out");
        };
        assert_eq!(polls, 2);
        assert_eq!(probe.packet_and_signal_slot().unwrap(), (41, 0));
        assert_eq!(backend.observe_calls, 2);
        assert_eq!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );
    }

    #[test]
    fn barrier_probe_currentness_fault_and_reset_failures_poison() {
        let mut currentness = owner();
        let probe = publish_barrier(&mut currentness);
        let mut backend = MockBackend::pending();
        backend.fail_currentness_at = Some(1);
        assert!(matches!(
            currentness.observe_barrier_probe_once(probe, &mut backend),
            Err(Gfx942CompletionErrorV1::Currentness)
        ));
        assert!(matches!(
            currentness.bind_barrier_probe(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        ));

        let mut fault = owner();
        let probe = publish_barrier(&mut fault);
        let mut backend = MockBackend::pending();
        backend.values[0] = -7;
        assert!(matches!(
            fault.observe_barrier_probe_once(probe, &mut backend),
            Err(Gfx942CompletionErrorV1::Fault { slot: 0, value: -7 })
        ));
        assert!(matches!(
            fault.bind_barrier_probe(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        ));

        let mut reset = owner();
        let probe = publish_barrier(&mut reset);
        let mut backend = MockBackend::pending();
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        let Gfx942BarrierProbePollV1::Ready { completed, .. } = reset
            .observe_barrier_probe_once(probe, &mut backend)
            .unwrap()
        else {
            panic!("completed probe remained pending");
        };
        backend.fail_reset_at = Some(1);
        assert_eq!(
            reset.recycle_barrier_probe(completed, &mut backend),
            Err(Gfx942CompletionErrorV1::Recycle)
        );
        assert!(matches!(
            reset.bind_barrier_probe(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        ));
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
        for count in [1_usize, 2, 4, 16, 256, 8192] {
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
                8192 => {
                    let values: CompletionPacketTemplatesV1<8192> =
                        CompletionPacketTemplatesV1::try_from_vec(templates).unwrap();
                    assert!(owner.bind_fixed_batch(values).is_ok());
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
        let over_values: CompletionPacketTemplatesV1<8193> =
            CompletionPacketTemplatesV1::try_from_vec(
                (0..8193).map(|index| template(index as u64)).collect(),
            )
            .unwrap();
        assert!(matches!(
            over.bind_fixed_batch(over_values),
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
        assert_eq!(capture.headers, vec![0x1502; 4]);
    }

    #[test]
    fn completion_binding_preserves_mixed_packet_ordering() {
        let mut owner = owner();
        let mut independent = template(0);
        independent.ordering = AqlDispatchOrderingV1::Independent;
        let bound = owner.bind_batch([independent, template(1)]).unwrap();
        let (packets, _) = bound.into_parts();
        let mut capture = PacketCapture::default();
        packets.publish_with(&mut capture).unwrap();
        assert_eq!(capture.headers, vec![0x1402, 0x1502]);
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
        wrong_vm.generations.code.allocation.vm = vm(3, 12);
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
    fn progress_uses_one_currentness_envelope_for_the_exact_batch_scan() {
        let mut owner = owner();
        let batch = publish(
            &mut owner,
            [template(0), template(1), template(2), template(3)],
        );
        let mut backend = MockBackend::pending();
        backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        backend.values[2] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        let batch = match owner
            .observe_once_with_progress(batch, &mut backend)
            .unwrap()
        {
            Gfx942CompletionPollWithProgressV1::Pending { batch, progress } => {
                assert_eq!(progress.packet_count(), 4);
                assert_eq!(progress.completed_count(), 2);
                assert_eq!(progress.pending_count(), 2);
                assert_eq!(progress.first_pending_batch_index(), Some(1));
                batch
            }
            Gfx942CompletionPollWithProgressV1::Ready { .. } => {
                panic!("partially completed batch reported ready")
            }
        };
        assert_eq!(backend.currentness_calls, 2);
        assert_eq!(backend.observe_calls, 4);

        backend.values[..4].fill(AMD_SIGNAL_VALUE_COMPLETE_V1);
        match owner
            .observe_once_with_progress(batch, &mut backend)
            .unwrap()
        {
            Gfx942CompletionPollWithProgressV1::Ready { progress, .. } => {
                assert_eq!(progress.packet_count(), 4);
                assert_eq!(progress.completed_count(), 4);
                assert_eq!(progress.pending_count(), 0);
                assert_eq!(progress.first_pending_batch_index(), None);
            }
            Gfx942CompletionPollWithProgressV1::Pending { .. } => {
                panic!("completed batch reported pending")
            }
        }
        assert_eq!(backend.currentness_calls, 4);
        assert_eq!(backend.observe_calls, 8);
    }

    #[test]
    fn ready_currentness_handoff_removes_exactly_one_recycle_opening_check() {
        let mut fused_owner = owner();
        let fused_batch = publish(&mut fused_owner, [template(0)]);
        let mut fused_backend = MockBackend::pending();
        fused_backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        let handoff = match fused_owner
            .observe_one_with_progress_current_handoff_retaining(fused_batch, &mut fused_backend)
            .unwrap()
        {
            CompletionPollWithCurrentnessHandoffV1::Ready { handoff, progress } => {
                assert_eq!(progress.completed_count(), 1);
                handoff
            }
            CompletionPollWithCurrentnessHandoffV1::Pending { .. } => {
                panic!("completed batch remained pending")
            }
        };
        fused_backend.trace.push("dispatch-completed");
        fused_backend.trace.push("allocation-completed");
        let recycled = fused_owner
            .recycle_current_handoff_retaining(handoff, &mut fused_backend)
            .unwrap();
        fused_backend.trace.push("dispatch-recycled");
        fused_backend.trace.push("attachment-recycled");
        assert_eq!(recycled.packet_count(), 1);
        assert_eq!(
            fused_backend.trace,
            [
                "currentness",
                "acquire",
                "currentness",
                "dispatch-completed",
                "allocation-completed",
                "reset",
                "currentness",
                "dispatch-recycled",
                "attachment-recycled",
            ]
        );
        assert_eq!(fused_backend.currentness_calls, 3);
        assert_eq!(fused_backend.observe_calls, 1);
        assert_eq!(fused_backend.reset_calls, 1);
        fused_owner.ensure_releasable().unwrap();

        let mut split_owner = owner();
        let split_batch = publish(&mut split_owner, [template(0)]);
        let mut split_backend = MockBackend::pending();
        split_backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
        let completed = match split_owner
            .observe_once_with_progress_retaining(split_batch, &mut split_backend)
            .unwrap()
        {
            Gfx942CompletionPollWithProgressV1::Ready { completed, .. } => completed,
            Gfx942CompletionPollWithProgressV1::Pending { .. } => {
                panic!("completed split batch remained pending")
            }
        };
        split_owner
            .recycle_retaining(completed, &mut split_backend)
            .unwrap();
        assert_eq!(split_backend.currentness_calls, 4);
        assert_eq!(split_backend.observe_calls, 1);
        assert_eq!(split_backend.reset_calls, 1);
    }

    #[test]
    fn pending_currentness_handoff_preserves_the_two_check_no_reset_path() {
        let mut owner = owner();
        let batch = publish(&mut owner, [template(0)]);
        let mut backend = MockBackend::pending();
        let pending = owner
            .observe_one_with_progress_current_handoff_retaining(batch, &mut backend)
            .unwrap();
        assert!(matches!(
            pending,
            CompletionPollWithCurrentnessHandoffV1::Pending { .. }
        ));
        assert_eq!(backend.trace, ["currentness", "acquire", "currentness"]);
        assert_eq!(backend.currentness_calls, 2);
        assert_eq!(backend.observe_calls, 1);
        assert_eq!(backend.reset_calls, 0);
    }

    #[test]
    fn currentness_handoff_failures_never_report_false_recycle() {
        for fail_currentness_at in [1_usize, 2] {
            let mut owner = owner();
            let batch = publish(&mut owner, [template(0)]);
            let mut backend = MockBackend::pending();
            backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
            backend.fail_currentness_at = Some(fail_currentness_at);
            assert!(matches!(
                owner.observe_one_with_progress_current_handoff_retaining(batch, &mut backend),
                Err((Gfx942CompletionErrorV1::Currentness, _))
            ));
            assert_eq!(backend.reset_calls, 0);
            assert!(matches!(
                owner.ensure_releasable(),
                Err(Gfx942CompletionErrorV1::Poisoned)
            ));
        }

        for (fail_reset_at, fail_currentness_at, expected, expected_trace) in [
            (
                Some(1_usize),
                None,
                Gfx942CompletionErrorV1::Recycle,
                &["currentness", "acquire", "currentness", "reset"][..],
            ),
            (
                None,
                Some(3_usize),
                Gfx942CompletionErrorV1::Currentness,
                &[
                    "currentness",
                    "acquire",
                    "currentness",
                    "reset",
                    "currentness",
                ][..],
            ),
        ] {
            let mut owner = owner();
            let batch = publish(&mut owner, [template(0)]);
            let mut backend = MockBackend::pending();
            backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
            let handoff = match owner
                .observe_one_with_progress_current_handoff_retaining(batch, &mut backend)
                .unwrap()
            {
                CompletionPollWithCurrentnessHandoffV1::Ready { handoff, .. } => handoff,
                CompletionPollWithCurrentnessHandoffV1::Pending { .. } => unreachable!(),
            };
            backend.fail_reset_at = fail_reset_at;
            backend.fail_currentness_at = fail_currentness_at;
            let (error, handoff) = owner
                .recycle_current_handoff_retaining(handoff, &mut backend)
                .unwrap_err();
            assert_eq!(error, expected);
            assert_eq!(handoff.into_completed().retention.batch_id, 1);
            assert_eq!(backend.trace, expected_trace);
            assert_eq!(backend.reset_calls, 1);
            assert!(matches!(
                owner.ensure_releasable(),
                Err(Gfx942CompletionErrorV1::Poisoned)
            ));
        }
    }

    #[test]
    fn substituted_handoff_identity_is_rejected_before_any_reset() {
        for substitution in 0..4 {
            let mut owner = owner();
            let mut batch = publish(&mut owner, [template(0)]);
            match substitution {
                0 => batch.retention.queue.generation = QueueGenerationV1(6),
                1 => batch.retention.signal_mapping.id = MappingIdV1(99),
                2 => batch.retention.batch_id = 99,
                3 => batch.retention.slots[0].generation += 1,
                _ => unreachable!(),
            }
            let mut backend = MockBackend::pending();
            backend.values[0] = AMD_SIGNAL_VALUE_COMPLETE_V1;
            assert!(matches!(
                owner.observe_one_with_progress_current_handoff_retaining(batch, &mut backend),
                Err((Gfx942CompletionErrorV1::StaleBatchGeneration, _))
            ));
            assert_eq!(backend.currentness_calls, 0);
            assert_eq!(backend.observe_calls, 0);
            assert_eq!(backend.reset_calls, 0);
        }
    }

    #[test]
    fn completion_currentness_handoff_is_private_and_move_only() {
        let production = include_str!("queue_completion.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let handoff = production
            .split("pub(super) struct CompletionCurrentnessHandoffV1")
            .nth(1)
            .unwrap()
            .split("pub(super) enum CompletionPollWithCurrentnessHandoffV1")
            .next()
            .unwrap();
        assert!(!handoff.contains("derive(Clone"));
        assert!(!handoff.contains("derive(Copy"));
        assert!(!production.contains("pub struct CompletionCurrentnessHandoffV1"));

        let specialized = production
            .split("fn observe_one_with_progress_current_handoff_retaining")
            .nth(1)
            .unwrap()
            .split("pub(super) fn recycle_current_handoff_retaining")
            .next()
            .unwrap();
        assert!(specialized.contains("backend.observe_one_acquire("));
        assert!(specialized.contains("self.validate_observation_preflight(&batch)"));
        assert!(specialized.contains(
            "self.classify_completion_observations(batch, core::iter::once(observation))"
        ));
        assert!(!specialized.contains("Vec<"));
        assert!(!specialized.contains("Vec::"));
        assert!(!specialized.contains("collect()"));
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

        let mut malformed_owner = owner();
        let malformed_batch = publish(&mut malformed_owner, [template(0)]);
        let mut malformed_backend = MockBackend::pending();
        malformed_backend.values[0] = -7;
        malformed_backend.extra_batch_observation = Some(AqlCompletionObservationV1::Completed);
        assert!(matches!(
            malformed_owner.observe_once(malformed_batch, &mut malformed_backend),
            Err(Gfx942CompletionErrorV1::Observation)
        ));
        assert_eq!(
            malformed_owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );

        let mut timeout_owner = owner();
        let timeout_batch = publish(&mut timeout_owner, [template(0)]);
        let mut timeout_backend = MockBackend::pending();
        let timeout = timeout_owner
            .wait_bounded(timeout_batch, 3, &mut timeout_backend)
            .unwrap_err();
        let Gfx942CompletionWaitFailureV1::Timeout { batch, polls } = timeout else {
            panic!("pending exhaustion did not preserve timeout custody")
        };
        assert_eq!(polls, 3);
        assert_eq!(batch.first_packet_and_signal_slot().unwrap(), (99, 0));
        assert_eq!(timeout_backend.observe_calls, 3);
        timeout_owner.poison_owner();

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
    fn zero_poll_timeout_preserves_locator_without_scanning_signals() {
        let mut owner = owner();
        let first = publish(&mut owner, [template(0), template(1)]);
        let mut backend = MockBackend::pending();
        backend.values[..2].fill(AMD_SIGNAL_VALUE_COMPLETE_V1);
        let completed = owner.wait_bounded(first, 1, &mut backend).unwrap();
        owner.recycle(completed, &mut backend).unwrap();

        let second = publish(&mut owner, [template(2), template(3), template(4)]);
        let scans_before = backend.observe_calls;
        let timeout = owner.wait_bounded(second, 0, &mut backend).unwrap_err();
        let Gfx942CompletionWaitFailureV1::Timeout { batch, polls } = timeout else {
            panic!("zero poll did not retain timeout custody")
        };
        assert_eq!(polls, 0);
        assert_eq!(batch.first_packet_and_signal_slot().unwrap(), (97, 0));
        assert_eq!(backend.observe_calls, scans_before);
        owner.poison_owner();
    }

    #[test]
    fn expired_deadline_preserves_custody_without_scanning_signals() {
        let mut owner = owner();
        let batch = publish(&mut owner, [template(0)]);
        let mut backend = MockBackend::pending();
        let timeout = owner
            .wait_until(batch, Instant::now(), &mut backend)
            .unwrap_err();
        let Gfx942CompletionWaitFailureV1::Timeout { batch, polls } = timeout else {
            panic!("expired deadline did not retain timeout custody")
        };
        assert_eq!(polls, 0);
        assert_eq!(backend.observe_calls, 0);
        assert_eq!(batch.first_packet_and_signal_slot().unwrap(), (99, 0));
        owner.poison_owner();
    }

    #[test]
    fn timeout_observation_is_addressless_and_preserves_exact_values() {
        let observation = Gfx942TimeoutExecutionObservationV1::new(
            545,
            545,
            0,
            0x1502,
            3,
            fe2o3_aql::AMD_SIGNAL_KIND_USER_V1,
            Gfx942TimeoutSignalObservationV1::Pending,
            0,
        );
        assert_eq!(observation.packet_count(), 545);
        assert_eq!(observation.write_counter(), 545);
        assert_eq!(observation.read_counter(), 0);
        assert_eq!(observation.first_packet_header(), 0x1502);
        assert_eq!(observation.first_packet_setup(), 3);
        assert_eq!(
            observation.first_signal_kind(),
            fe2o3_aql::AMD_SIGNAL_KIND_USER_V1
        );
        assert_eq!(
            observation.first_signal(),
            Gfx942TimeoutSignalObservationV1::Pending
        );
        assert_eq!(observation.first_signal().value(), 1);
        assert_eq!(observation.queue_exception_reason_mask(), 0);
        assert!(observation.currentness_confirmed());
        let rendered = format!("{observation:?}");
        for forbidden in ["address", "handle", "queue_id", "packet_id", "slot_index"] {
            assert!(!rendered.contains(forbidden));
        }
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
        let all: CompletionPacketTemplatesV1<8192> = CompletionPacketTemplatesV1::try_from_vec(
            (0..8192).map(|index| template(index as u64)).collect(),
        )
        .unwrap();
        assert!(full.bind_fixed_batch(all).is_ok());
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
            Err(Gfx942CompletionWaitFailureV1::Terminal(
                Gfx942CompletionErrorV1::InvalidPollBound {
                    requested,
                    maximum: MAX_COMPLETION_POLL_ATTEMPTS_V1,
                }
            )) if requested == MAX_COMPLETION_POLL_ATTEMPTS_V1 + 1
        ));
        assert_eq!(backend.observe_calls, 0);
        assert_eq!(
            owner.ensure_releasable(),
            Err(Gfx942CompletionErrorV1::Poisoned)
        );
    }
}

//! Private, bounded completion authority for the retained compute-AQL queue.
//!
//! Every fixed-batch packet and the isolated barrier probe receive distinct
//! ROCr user signals from one coherent GTT arena. Numeric addresses remain
//! inside this module. Fixed-batch state retains exact dispatch generations;
//! the barrier probe retains only its exact queue and signal generations until
//! completion has been observed and explicitly recycled.

use core::fmt;
use core::hash::{Hash, Hasher};
use std::time::Instant;

use fe2o3_aql::{
    AMD_SIGNAL_ALIGNMENT_V1, AMD_SIGNAL_BYTES_V1, AQL_MAX_FIXED_BATCH_PACKETS_V2,
    AmdBusyCompletionSignalV1, AqlBarrierAndPacketErrorV1, AqlBarrierAndPacketV1,
    AqlCompletionObservationV1, AqlDependencyDispatchPlanErrorV1, AqlDependencySignalObservationV1,
    AqlDispatchGeometryV1, AqlDispatchOrderingV1, AqlDispatchPacketError,
    AqlKernelDispatchPacketV1, AqlPreparedDependencyDispatchV1,
    AqlPreparedKernelDispatchBatchErrorV1, AqlPreparedKernelDispatchBatchV2,
    AqlPreparedKernelDispatchV1, ObservedGpuAddressV1,
};
use fe2o3_runtime_model::{MemoryMappingKeyV1, QueueKeyV1};
use sha2::{Digest, Sha256};

use crate::shared_memory::SharedGttMappedResourceFactsV1;
use crate::wait::MonotonicWaitV1;

include!("queue_completion/event_release_body.rs");
include!("queue_completion/bound_cancel_body.rs");
include!("queue_completion/rollback_adapters_body.rs");
include!("queue_completion/event_bind_body.rs");
include!("queue_completion/event_issue_body.rs");
include!("queue_completion/batch_bind_body.rs");
include!("queue_dispatch_binding/template_prepare_body.rs");
include!("queue_completion/dispatch_roster_body.rs");

macro_rules! completion_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[path = "queue_completion/dependency_event.rs"]
mod dependency_event;

use dependency_event::{
    CompletionDependencyLedgerV1, Gfx942ComputeDependencyReaderBatchFailureV1,
    Gfx942ComputeDependencyReaderBatchV1, Gfx942ComputeEventBatchFailureV1,
};
pub use dependency_event::{
    GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_SHA256_V1, GFX942_COMPUTE_EVENT_CUSTODY_MANIFEST_V1,
    GFX942_MAX_COMPUTE_DEPENDENCY_READERS_V1, GFX942_MAX_COMPUTE_EVENT_OCCURRENCES_V1,
    Gfx942ComputeDependencyReaderLeaseV1, Gfx942ComputeDependencyReaderReleaseObservationV1,
    Gfx942ComputeEventBindingStateV1, Gfx942ComputeEventOccurrenceV1,
    Gfx942ComputeEventReleaseObservationV1,
};

pub(crate) const COMPLETION_SIGNAL_CAPACITY_V1: usize = AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize;
pub(crate) const COMPLETION_SIGNAL_ARENA_BYTES_V1: usize =
    COMPLETION_SIGNAL_CAPACITY_V1 * AMD_SIGNAL_BYTES_V1;
pub(super) const MAX_COMPLETION_POLL_ATTEMPTS_V1: u32 = 1_000_000;

/// Canonical claim boundary for the private completion-signal slice.
pub const GFX942_AQL_COMPLETION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-aql-completion-r52-v1\n",
    "aql_dispatch_schema_sha256=82fbd7cf0b6c8647dce3f9b11e4f13a2dadfe3423509f769a4bc6cc87bb7acd0\n",
    "aql_barrier_and_schema_sha256=bdca900cd5c6eaccbddfc5a854e956382a08ce87bec4ccd5284baacf932cdfb5\n",
    "aql_fixed_batch_schema_sha256=a3c74fe4aa26a62772253de267812f2fb1626247685d8c4e8ed8bbb2a5a9e34a\n",
    "compute_event_custody_schema_sha256=3b235c35d117c198fcb21f65197431a47de78ed5081b95b459bbde61c6410e9a\n",
    "arena=one-host-visible-coherent-gtt-allocation,524288-bytes,8192-distinct-64-byte-aligned-user-signals\n",
    "batch=1-through-8192,heap-owned-fixed-cardinality-state,one-unique-signal-per-packet,no-aggregate-alias,one-sha256-occurrence-commitment-over-exact-batch-id-queue-signal-mapping-ordered-slot-indices-and-generations-dispatch-roster-packet-count-and-first-last-packet-identities\n",
    "initialization=typed-amd-busy-signal-construction,kind-user-1,value-pending-1,event-fields-zero,before-gpu-map\n",
    "fixed-batch-binding=crate-private-packet-construction,per-packet-independent-or-wait-for-prior-ordering-retained,no-public-signal-address,exact-queue-vm-signal-code-kernarg-and-nonzero-dispatch-generations-retained,actual-resource-lifetimes-owned-by-private-c5-queue-owner\n",
    "observation=monotonic-deadline-or-legacy-bounded-poll,short-spin-then-yield-and-bounded-exponential-sleep,one-pre-post-currentness-envelope-around-one-exact-retained-signal-set-of-atomic-i64-acquire-loads,same-scan-redacted-packet-completed-pending-and-first-pending-index-progress,all-retained-signals-zero-before-ready,unexpected-value-is-fault,timeout-retains-linear-operation-privately-until-addressless-counter-first-retained-packet-first-retained-signal-exception-currentness-snapshot\n",
    "event-custody=addressless-exact-occurrence-and-native-reader-ledgers,bounded-8192-each,complete-batch-atomic-record-bind-retain-and-release,independent-checked-event-and-reader-pin-counts,drop-inert\n",
    "validation=fixed-8192-slot-bitmap-with-one-pass-linear-retention-slot-uniqueness-check-and-one-pass-linear-exact-dispatch-roster-validation\n",
    "recycle=fixed-batch-only-after-exact-all-signal-completion-and-zero-event-and-reader-pins-or-barrier-probe-only-after-exact-one-signal-completion,atomic-i64-release-reset-to-pending,checked-slot-generation-increment\n",
    "barrier-probe=isolated-owner-phase,exact-one-slot,queue-and-signal-generations-only,no-code-kernarg-or-dispatch-generation,bound-published-completed-recycled-linear-custody,zero-dependency-system-scope-header-0x1403\n",
    "failure=currentness-native-observation-unexpected-value-timeout-invalid-poll-bound-generation-exhaustion-or-reset-ambiguity-poisons-owner-and-queue;timeout-snapshot-precedes-poison-and-grants-no-native-authority;teardown-required\n",
    "release=queue-destroy-first,only-when-every-batch-was-completed-and-recycled-and-event-reader-ledgers-are-empty,explicit-unmap-and-free,no-drop-native-effects\n",
    "proof=host-state-machine-and-mock-fault-tests-only,sha256-collision-resistance-cpu-gpu-atomic-coherence-device-write-visibility-firmware-signal-and-quiescence-refinement-contracted\n",
    "excluded=public-safe-launch,dependency-packet-publication,resource-lifetime-mint,copy,alias-proof,formal-collision-free-occurrence-proof,hardware-execution,ioctl-validation\n",
);

/// SHA-256 of [`GFX942_AQL_COMPLETION_MANIFEST_V1`].
pub const GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1: &str =
    "485b21257623afce41573b27922350f125bcd7f5d339f8a89cf9a2c7c6ca77f1";

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
    event_pins: u32,
    native_reader_pins: u32,
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
            event_pins: 0,
            native_reader_pins: 0,
        },
    );
    slots
        .into_boxed_slice()
        .try_into()
        .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("completion state cardinality"))
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CompletionDispatchGenerationBindingV1 {
    queue: QueueKeyV1,
    code: MemoryMappingKeyV1,
    kernarg: MemoryMappingKeyV1,
    dispatch_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CompletionBatchOccurrenceV1 {
    pub(super) batch_id: u64,
    pub(super) queue: QueueKeyV1,
    pub(super) signal_mapping: MemoryMappingKeyV1,
    pub(super) packet_count: usize,
    pub(super) first_packet_id: u64,
    pub(super) last_packet_id: u64,
    pub(super) roster_sha256: [u8; 32],
    pub(super) dispatch_roster: CompletionDispatchRosterV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CompletionDispatchRosterV1 {
    pub(super) queue: QueueKeyV1,
    pub(super) packet_count: usize,
    pub(super) dispatch_generation: u64,
    pub(super) roster_sha256: [u8; 32],
}

struct CompletionOccurrenceHasherV1(Sha256);

impl Hasher for CompletionOccurrenceHasherV1 {
    fn finish(&self) -> u64 {
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
}

pub(super) fn completion_dispatch_roster_v1(
    dispatches: &[CompletionDispatchGenerationBindingV1],
) -> Result<CompletionDispatchRosterV1, Gfx942CompletionErrorV1> {
    completion_dispatch_roster_with_visits_v1(dispatches).map(|(roster, _)| roster)
}

fn completion_dispatch_roster_with_visits_v1(
    dispatches: &[CompletionDispatchGenerationBindingV1],
) -> Result<(CompletionDispatchRosterV1, usize), Gfx942CompletionErrorV1> {
    let mut visits = 0;
    let roster = completion_dispatch_roster_projected_v1(dispatches, |dispatch| {
        visits += 1;
        *dispatch
    })?;
    Ok((roster, visits))
}

pub(super) fn completion_template_dispatch_roster_v1(
    templates: &[CompletionPacketTemplateV1],
) -> Result<CompletionDispatchRosterV1, Gfx942CompletionErrorV1> {
    completion_dispatch_roster_projected_v1(templates, |template| template.generations())
}

fn completion_dispatch_roster_projected_v1<T>(
    values: &[T],
    project: impl FnMut(&T) -> CompletionDispatchGenerationBindingV1,
) -> Result<CompletionDispatchRosterV1, Gfx942CompletionErrorV1> {
    let mut hasher = CompletionOccurrenceHasherV1(Sha256::new());
    let (queue, dispatch_generation) =
        hash_completion_dispatch_roster_projected_v1(values, project, &mut hasher)?;
    Ok(CompletionDispatchRosterV1 {
        queue,
        packet_count: values.len(),
        dispatch_generation,
        roster_sha256: hasher.0.finalize().into(),
    })
}

fn hash_completion_dispatch_roster_projected_v1<T, H: Hasher>(
    values: &[T],
    mut project: impl FnMut(&T) -> CompletionDispatchGenerationBindingV1,
    hasher: &mut H,
) -> Result<(QueueKeyV1, u64), Gfx942CompletionErrorV1> {
    completion_hash_roster_body!(completion_rust_expr, values, project, hasher)
}

fn completion_batch_occurrence_v1<const N: usize>(
    retention: &CompletionBatchRetentionV1<N>,
) -> Result<CompletionBatchOccurrenceV1, Gfx942CompletionErrorV1> {
    let last_packet_id = retention
        .last_packet_id
        .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
    let packet_count =
        u64::try_from(N).map_err(|_| Gfx942CompletionErrorV1::PacketCountExceedsMaximum {
            requested: N,
            maximum: AQL_MAX_FIXED_BATCH_PACKETS_V2 as usize,
        })?;
    let first_packet_id = last_packet_id
        .checked_add(1)
        .and_then(|next| next.checked_sub(packet_count))
        .ok_or(Gfx942CompletionErrorV1::StaleBatchGeneration)?;
    let dispatch_roster = completion_dispatch_roster_v1(&*retention.dispatches)?;
    if dispatch_roster.queue != retention.queue || dispatch_roster.packet_count != N {
        return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
    }
    let mut hasher = CompletionOccurrenceHasherV1(Sha256::new());
    retention.batch_id.hash(&mut hasher);
    retention.queue.hash(&mut hasher);
    retention.signal_mapping.hash(&mut hasher);
    retention.slots.hash(&mut hasher);
    retention.dispatches.hash(&mut hasher);
    N.hash(&mut hasher);
    first_packet_id.hash(&mut hasher);
    last_packet_id.hash(&mut hasher);
    Ok(CompletionBatchOccurrenceV1 {
        batch_id: retention.batch_id,
        queue: retention.queue,
        signal_mapping: retention.signal_mapping,
        packet_count: N,
        first_packet_id,
        last_packet_id,
        roster_sha256: hasher.0.finalize().into(),
        dispatch_roster,
    })
}

impl CompletionDispatchGenerationBindingV1 {
    #[allow(dead_code)]
    pub(crate) const fn new(
        queue: QueueKeyV1,
        code: MemoryMappingKeyV1,
        kernarg: MemoryMappingKeyV1,
        dispatch_generation: u64,
    ) -> Self {
        dispatch_template_generation_new_body!(
            completion_rust_expr,
            queue,
            code,
            kernarg,
            dispatch_generation
        )
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
    #[cfg(any(test, feature = "cpu-runtime-fixtures"))]
    fn from_array(values: [CompletionPacketTemplateV1; N]) -> Self {
        Self {
            values: Box::new(values),
        }
    }

    #[cfg(any(test, feature = "cpu-runtime-fixtures"))]
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
        dispatch_template_new_body!(
            completion_rust_expr,
            geometry,
            ordering,
            private_segment_size,
            group_segment_size,
            kernel_object,
            kernarg_address,
            kernarg_alignment,
            generations
        )
    }

    pub(super) const fn generations(self) -> CompletionDispatchGenerationBindingV1 {
        self.generations
    }

    #[cfg(test)]
    pub(super) const fn ordering_for_test(self) -> AqlDispatchOrderingV1 {
        self.ordering
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct CompletionSlotLeaseV1 {
    index: u32,
    generation: u64,
}

/// Crate-private, addressless identity for one exact completion occurrence.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct ComputeDependencyOccurrenceIdentityV1 {
    pub(super) session_occurrence: u64,
    pub(super) acceptance_epoch: u64,
    pub(super) batch_id: u64,
    pub(super) queue: QueueKeyV1,
    pub(super) signal_mapping: MemoryMappingKeyV1,
    pub(super) slot_index: u32,
    pub(super) slot_generation: u64,
    pub(super) dispatch_generation: u64,
    pub(super) packet_id: Option<u64>,
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

impl<const N: usize> fmt::Debug for BoundCompletionBatchV1<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BoundCompletionBatchV1")
            .field("packet_count", &N)
            .finish_non_exhaustive()
    }
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

/// Sealed prepublication target derived from one exact bound completion batch.
pub(super) struct PreparedComputeDependencyTargetV1 {
    identity: ComputeDependencyOccurrenceIdentityV1,
    retention: CompletionBatchRetentionV1<1>,
    event: Gfx942ComputeEventOccurrenceV1,
    final_dispatch: AqlPreparedKernelDispatchV1,
}

impl fmt::Debug for PreparedComputeDependencyTargetV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedComputeDependencyTargetV1")
            .finish_non_exhaustive()
    }
}

impl PreparedComputeDependencyTargetV1 {
    pub(super) const fn identity(&self) -> ComputeDependencyOccurrenceIdentityV1 {
        self.identity
    }
}

pub(super) struct PlannedComputeDependencyTargetV1 {
    identity: ComputeDependencyOccurrenceIdentityV1,
    retention: CompletionBatchRetentionV1<1>,
    event: Gfx942ComputeEventOccurrenceV1,
    plan: AqlPreparedDependencyDispatchV1,
}

impl PlannedComputeDependencyTargetV1 {
    pub(super) fn into_parts(
        self,
    ) -> (
        ComputeDependencyOccurrenceIdentityV1,
        CompletionBatchRetentionV1<1>,
        Gfx942ComputeEventOccurrenceV1,
        AqlPreparedDependencyDispatchV1,
    ) {
        (self.identity, self.retention, self.event, self.plan)
    }
}

#[derive(Debug)]
pub(super) enum ComputeDependencyTargetPlanErrorV1 {
    Completion(Gfx942CompletionErrorV1),
    Plan(AqlDependencyDispatchPlanErrorV1),
}

#[cfg(test)]
#[derive(Clone, Copy)]
pub(super) enum ComputeDependencyTargetSubstitutionV1 {
    FinalDispatch,
    Retention,
    Event,
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
    pub(super) fn occurrence_v1(
        &self,
    ) -> Result<CompletionBatchOccurrenceV1, Gfx942CompletionErrorV1> {
        completion_batch_occurrence_v1(&self.retention)
    }

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

impl<const N: usize> Gfx942CompletedBatchV1<N> {
    pub(super) fn occurrence_v1(
        &self,
    ) -> Result<CompletionBatchOccurrenceV1, Gfx942CompletionErrorV1> {
        completion_batch_occurrence_v1(&self.retention)
    }
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
    EventIdentityExhausted,
    EventCapacityExhausted,
    DependencyReaderIdentityExhausted,
    DependencyReaderCapacityExhausted,
    SignalPinCountExhausted,
    InvalidSessionOccurrence,
    InvalidAcceptanceEpoch,
    EventAlreadyBound,
    EventNotPublished,
    CrossSessionEvent,
    SelfDependency,
    DependencyCycle,
    DuplicateDependency,
    DependencyLedgerAllocation,
    StaleEventOccurrence,
    StaleDependencyReader,
    SignalPinned {
        slot: u32,
        event_pins: u32,
        native_reader_pins: u32,
    },
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

#[cfg(test)]
#[derive(Clone, Copy)]
pub(super) enum TestOnlyAmbiguousCompletionObservationV1 {
    Pending,
    Completed,
}

#[cfg(test)]
struct TestOnlyFalseClosingCurrentnessBackendV1 {
    observation: TestOnlyAmbiguousCompletionObservationV1,
    currentness_checks: u8,
    closing_current: bool,
}

#[cfg(test)]
impl NativeCompletionSignalBackendV1 for TestOnlyFalseClosingCurrentnessBackendV1 {
    fn check_currentness(&mut self) -> Result<(), Gfx942CompletionErrorV1> {
        self.currentness_checks += 1;
        if self.currentness_checks == 2 && !self.closing_current {
            Err(Gfx942CompletionErrorV1::Currentness)
        } else {
            Ok(())
        }
    }

    fn observe_one_acquire_in_current_scope(
        &mut self,
        _slot_index: u32,
    ) -> Result<AqlCompletionObservationV1, Gfx942CompletionErrorV1> {
        Ok(match self.observation {
            TestOnlyAmbiguousCompletionObservationV1::Pending => {
                AqlCompletionObservationV1::Pending
            }
            TestOnlyAmbiguousCompletionObservationV1::Completed => {
                AqlCompletionObservationV1::Completed
            }
        })
    }

    fn observe_batch_acquire_in_current_scope(
        &mut self,
        _slot_indices: &[u32],
    ) -> Result<Vec<AqlCompletionObservationV1>, Gfx942CompletionErrorV1> {
        unreachable!("three-binding persistent compute observes one packet")
    }

    fn reset_pending_release(&mut self, _slot_index: u32) -> Result<(), Gfx942CompletionErrorV1> {
        unreachable!("false closing currentness cannot reach recycle")
    }
}

pub(super) struct CompletionSignalArenaOwnerV1 {
    queue: QueueKeyV1,
    signal_mapping: MemoryMappingKeyV1,
    gpu_base: u64,
    next_batch_id: u64,
    slots: Box<[CompletionSlotRecordV1; COMPLETION_SIGNAL_CAPACITY_V1]>,
    dependency_ledger: Box<CompletionDependencyLedgerV1>,
    phase: CompletionOwnerPhaseV1,
}

#[cfg(any(test, feature = "cpu-runtime-fixtures"))]
#[derive(Debug, Eq, PartialEq)]
pub(super) struct CompletionCustodySnapshotV1 {
    queue: QueueKeyV1,
    signal_mapping: MemoryMappingKeyV1,
    gpu_base: u64,
    next_batch_id: u64,
    slots: Vec<CompletionSlotRecordV1>,
    slot_storage: usize,
    dependency_storage: usize,
}

impl CompletionSignalArenaOwnerV1 {
    #[cfg(feature = "cpu-runtime-fixtures")]
    pub(super) fn saturate_cpu_fixture(
        &mut self,
        template: CompletionPacketTemplateV1,
        last_packet: u64,
    ) -> Result<Gfx942CompletionBatchV1<COMPLETION_SIGNAL_CAPACITY_V1>, Gfx942CompletionErrorV1>
    {
        self.ensure_releasable()?;
        let templates = CompletionPacketTemplatesV1::try_from_vec(vec![
                template;
                COMPLETION_SIGNAL_CAPACITY_V1
            ])
        .expect("fixed CPU fixture cardinality");
        let bound = self.bind_fixed_batch(templates)?;
        let (_, retention) = bound.into_parts();
        self.mark_published(retention, last_packet)
    }

    #[cfg(test)]
    pub(super) fn is_poisoned_for_test(&self) -> bool {
        self.phase == CompletionOwnerPhaseV1::Poisoned
    }

    #[cfg(any(test, feature = "cpu-runtime-fixtures"))]
    pub(super) fn custody_snapshot_for_test(&self) -> CompletionCustodySnapshotV1 {
        // Poisoning changes the owner's phase, not its exact slots or ledger storage.
        CompletionCustodySnapshotV1 {
            queue: self.queue,
            signal_mapping: self.signal_mapping,
            gpu_base: self.gpu_base,
            next_batch_id: self.next_batch_id,
            slots: self.slots.to_vec(),
            slot_storage: self.slots.as_ptr() as usize,
            dependency_storage: &*self.dependency_ledger as *const _ as usize,
        }
    }

    #[cfg(test)]
    pub(super) fn fill_all_signals_for_test(
        &mut self,
        template: CompletionPacketTemplateV1,
    ) -> Gfx942CompletionBatchV1<COMPLETION_SIGNAL_CAPACITY_V1> {
        let templates = CompletionPacketTemplatesV1::try_from_vec(vec![
            template;
            COMPLETION_SIGNAL_CAPACITY_V1
        ])
        .expect("fixed test cardinality");
        let bound = self.bind_fixed_batch(templates).unwrap();
        let (_, retention) = bound.into_parts();
        self.mark_published(
            retention,
            u64::try_from(COMPLETION_SIGNAL_CAPACITY_V1).unwrap(),
        )
        .unwrap()
    }

    #[cfg(test)]
    pub(super) fn complete_and_recycle_all_for_test(
        &mut self,
        batch: Gfx942CompletionBatchV1<COMPLETION_SIGNAL_CAPACITY_V1>,
    ) {
        self.validate_published(&batch.retention).unwrap();
        self.require_unpinned(&batch.retention.slots).unwrap();
        for slot in batch.retention.slots.iter() {
            let record = &mut self.slots[slot.index as usize];
            record.generation = record.generation.checked_add(1).unwrap();
            record.phase = CompletionSlotPhaseV1::Available;
        }
    }

    #[cfg(test)]
    pub(super) fn complete_one_without_native_for_test(
        &mut self,
        batch: Gfx942CompletionBatchV1<1>,
    ) -> Gfx942CompletedBatchV1<1> {
        self.validate_published(&batch.retention).unwrap();
        let slot = batch.retention.slots[0];
        self.slots[slot.index as usize].phase = CompletionSlotPhaseV1::Completed {
            batch_id: batch.retention.batch_id,
        };
        Gfx942CompletedBatchV1 {
            retention: batch.retention,
        }
    }

    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(super) fn observe_one_with_false_closing_currentness_for_test(
        &mut self,
        batch: Gfx942CompletionBatchV1<1>,
        observation: TestOnlyAmbiguousCompletionObservationV1,
    ) -> Result<
        CompletionPollWithCurrentnessHandoffV1<1>,
        (Gfx942CompletionErrorV1, Gfx942CompletionBatchV1<1>),
    > {
        let mut backend = TestOnlyFalseClosingCurrentnessBackendV1 {
            observation,
            currentness_checks: 0,
            closing_current: false,
        };
        let result = self.observe_one_with_progress_current_handoff_retaining(batch, &mut backend);
        debug_assert_eq!(backend.currentness_checks, 2);
        result
    }

    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(super) fn observe_one_pending_with_current_closing_for_test(
        &mut self,
        batch: Gfx942CompletionBatchV1<1>,
    ) -> Result<
        CompletionPollWithCurrentnessHandoffV1<1>,
        (Gfx942CompletionErrorV1, Gfx942CompletionBatchV1<1>),
    > {
        let mut backend = TestOnlyFalseClosingCurrentnessBackendV1 {
            observation: TestOnlyAmbiguousCompletionObservationV1::Pending,
            currentness_checks: 0,
            closing_current: true,
        };
        let result = self.observe_one_with_progress_current_handoff_retaining(batch, &mut backend);
        debug_assert_eq!(backend.currentness_checks, 2);
        result
    }

    #[cfg(test)]
    pub(super) fn recycle_one_without_native_for_test(
        &mut self,
        completed: Gfx942CompletedBatchV1<1>,
    ) -> Gfx942CompletionRecycleObservationV1 {
        self.validate_completed(&completed.retention).unwrap();
        self.require_unpinned(&completed.retention.slots).unwrap();
        let slot = completed.retention.slots[0];
        let record = &mut self.slots[slot.index as usize];
        record.generation = record.generation.checked_add(1).unwrap();
        record.phase = CompletionSlotPhaseV1::Available;
        Gfx942CompletionRecycleObservationV1 { packet_count: 1 }
    }

    #[cfg(any(test, feature = "cpu-runtime-fixtures"))]
    pub(super) fn state_snapshot_for_test(&self) -> (u64, usize) {
        (
            self.next_batch_id,
            self.slots
                .iter()
                .filter(|slot| slot.phase == CompletionSlotPhaseV1::Available)
                .count(),
        )
    }

    pub(super) fn record_dependency_event_batch_for_bound_v1<const N: usize>(
        &mut self,
        session_occurrence: u64,
        source_acceptance_epoch: u64,
        bound: &BoundCompletionBatchV1<N>,
    ) -> Result<Vec<Gfx942ComputeEventOccurrenceV1>, Gfx942CompletionErrorV1> {
        completion_record_bound_dependency_batch_body!(
            completion_rust_expr,
            self,
            session_occurrence,
            source_acceptance_epoch,
            bound
        )
    }

    /// Hostile phase injection, never a native publication or completion receipt.
    #[cfg(test)]
    pub(super) fn invalidate_bound_phase_for_test(&mut self) {
        let record = self
            .slots
            .iter_mut()
            .find(|record| matches!(record.phase, CompletionSlotPhaseV1::Bound { .. }))
            .expect("fault requires a retained bound slot");
        let CompletionSlotPhaseV1::Bound { batch_id } = record.phase else {
            unreachable!()
        };
        record.phase = CompletionSlotPhaseV1::Published { batch_id };
    }

    pub(super) fn record_dependency_event_batch_v1<const N: usize>(
        &mut self,
        session_occurrence: u64,
        source_acceptance_epoch: u64,
        retention: &CompletionBatchRetentionV1<N>,
    ) -> Result<Vec<Gfx942ComputeEventOccurrenceV1>, Gfx942CompletionErrorV1> {
        completion_record_dependency_batch_body!(
            completion_rust_expr,
            self,
            session_occurrence,
            source_acceptance_epoch,
            retention
        )
    }

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
            dependency_ledger: Box::new(CompletionDependencyLedgerV1::new()),
            phase: CompletionOwnerPhaseV1::Ready,
        })
    }

    #[cfg(any(test, feature = "cpu-runtime-fixtures"))]
    pub(super) fn for_persistent_compute_cancellation_test(queue: QueueKeyV1) -> Self {
        Self::for_dependency_test(queue, 1, AMD_SIGNAL_ALIGNMENT_V1 as u64)
    }

    #[cfg(any(test, feature = "cpu-runtime-fixtures"))]
    pub(super) fn for_dependency_test(queue: QueueKeyV1, arena_id: u64, gpu_base: u64) -> Self {
        assert!(gpu_base != 0 && gpu_base.is_multiple_of(AMD_SIGNAL_ALIGNMENT_V1 as u64));
        Self {
            queue,
            signal_mapping: MemoryMappingKeyV1 {
                allocation: fe2o3_runtime_model::MemoryAllocationKeyV1 {
                    vm: queue.vm,
                    id: fe2o3_runtime_model::AllocationIdV1(arena_id),
                    generation: fe2o3_runtime_model::AllocationGenerationV1(1),
                },
                id: fe2o3_runtime_model::MappingIdV1(arena_id),
            },
            gpu_base,
            next_batch_id: 1,
            slots: allocate_completion_slot_records_v1()
                .expect("fixed completion test roster is allocatable"),
            dependency_ledger: Box::new(CompletionDependencyLedgerV1::new()),
            phase: CompletionOwnerPhaseV1::Ready,
        }
    }

    #[cfg(any(test, feature = "cpu-runtime-fixtures"))]
    pub(super) fn bind_batch<const N: usize>(
        &mut self,
        templates: [CompletionPacketTemplateV1; N],
    ) -> Result<BoundCompletionBatchV1<N>, Gfx942CompletionErrorV1> {
        self.bind_fixed_batch(CompletionPacketTemplatesV1::from_array(templates))
    }

    pub(super) fn bind_boxed_batch<const N: usize>(
        &mut self,
        templates: Box<[CompletionPacketTemplateV1; N]>,
    ) -> Result<BoundCompletionBatchV1<N>, Gfx942CompletionErrorV1> {
        self.bind_fixed_batch(CompletionPacketTemplatesV1 { values: templates })
    }

    fn bind_fixed_batch<const N: usize>(
        &mut self,
        templates: CompletionPacketTemplatesV1<N>,
    ) -> Result<BoundCompletionBatchV1<N>, Gfx942CompletionErrorV1> {
        completion_bind_batch_body!(completion_rust_expr, self, templates, N)
    }

    fn select_available_slots(&self, count: usize) -> Vec<CompletionSlotLeaseV1> {
        completion_select_slots_body!(completion_rust_expr, self, count)
    }

    fn commit_bound_batch<const N: usize>(
        &mut self,
        bound: BoundCompletionBatchV1<N>,
        next_batch_id: u64,
    ) -> BoundCompletionBatchV1<N> {
        completion_commit_batch_body!(completion_rust_expr, self, bound, next_batch_id, N)
    }

    pub(super) fn validate_bound<const N: usize>(
        &self,
        retention: &CompletionBatchRetentionV1<N>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        completion_validate_bound_body!(completion_rust_expr, self, retention)
    }

    pub(super) fn cancel_bound<const N: usize>(
        &mut self,
        retention: CompletionBatchRetentionV1<N>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        completion_cancel_bound_body!(completion_rust_expr, self, retention)
    }

    /// Owner-preserving cancellation used when a prepublication event pin may
    /// make the otherwise no-effect batch temporarily unreleasable.
    #[allow(clippy::result_large_err)]
    pub(super) fn cancel_bound_retaining<const N: usize>(
        &mut self,
        retention: CompletionBatchRetentionV1<N>,
    ) -> Result<(), (Gfx942CompletionErrorV1, CompletionBatchRetentionV1<N>)> {
        completion_cancel_bound_retaining_body!(completion_rust_expr, self, retention, N)
    }

    pub(super) fn mark_published<const N: usize>(
        &mut self,
        retention: CompletionBatchRetentionV1<N>,
        last_packet_id: u64,
    ) -> Result<Gfx942CompletionBatchV1<N>, Gfx942CompletionErrorV1> {
        self.mark_published_retaining(retention, last_packet_id)
            .map_err(|(error, _retention)| error)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn mark_published_retaining<const N: usize>(
        &mut self,
        mut retention: CompletionBatchRetentionV1<N>,
        last_packet_id: u64,
    ) -> Result<Gfx942CompletionBatchV1<N>, (Gfx942CompletionErrorV1, CompletionBatchRetentionV1<N>)>
    {
        completion_mark_published_retaining_body!(
            completion_rust_expr,
            self,
            retention,
            last_packet_id,
            N
        )
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
        if let Err(error) = self.require_unpinned(&completed.retention.slots) {
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
        if let Err(error) = self.require_unpinned(&completed.retention.slots) {
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
        if !self.dependency_ledger.is_empty()
            || self.slots.iter().any(|record| {
                record.phase != CompletionSlotPhaseV1::Available
                    || record.event_pins != 0
                    || record.native_reader_pins != 0
            })
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
        completion_dispatch_binding_body!(completion_rust_expr, self, binding)
    }

    fn require_unpinned<const N: usize>(
        &self,
        slots: &[CompletionSlotLeaseV1; N],
    ) -> Result<(), Gfx942CompletionErrorV1> {
        completion_require_unpinned_body!(completion_rust_expr, self, slots, N)
    }

    fn validate_published<const N: usize>(
        &self,
        retention: &CompletionBatchRetentionV1<N>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        completion_validate_published_body!(completion_rust_expr, self, retention)
    }

    fn validate_completed<const N: usize>(
        &self,
        retention: &CompletionBatchRetentionV1<N>,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        if retention.last_packet_id.is_none() {
            return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
        }
        self.validate_retention(
            retention,
            CompletionSlotPhaseV1::Completed {
                batch_id: retention.batch_id,
            },
        )
    }

    fn validate_retention<const N: usize>(
        &self,
        retention: &CompletionBatchRetentionV1<N>,
        expected: CompletionSlotPhaseV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        completion_validate_retention_body!(completion_rust_expr, self, retention, expected, N)
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
        completion_require_ready_body!(completion_rust_expr, self)
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

// Parent-module bridges keep completion-event internals private to their
// behavior owner while allowing the sibling queue orchestrator to compose the
// exact linear transitions.
#[allow(dead_code)]
impl CompletionSignalArenaOwnerV1 {
    /// Couples one exact bound batch to its addressless target identity and
    /// newly recorded event before any native queue effect is possible.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_dependency_target_v1(
        &mut self,
        session_occurrence: u64,
        acceptance_epoch: u64,
        bound: BoundCompletionBatchV1<1>,
    ) -> Result<
        PreparedComputeDependencyTargetV1,
        (Gfx942CompletionErrorV1, BoundCompletionBatchV1<1>),
    > {
        let result = (|| {
            self.require_ready()?;
            self.validate_bound(&bound.retention)?;
            let expected_signal = self.dependency_target_signal_v1(&bound.retention)?;
            if !bound.packets.matches_one_completion_signal(expected_signal) {
                return Err(Gfx942CompletionErrorV1::InvalidArena(
                    "dependency target completion signal",
                ));
            }
            Ok(dependency_target_identity_v1(
                session_occurrence,
                acceptance_epoch,
                &bound.retention,
            ))
        })();
        let identity = match result {
            Ok(identity) => identity,
            Err(error) => return Err((error, bound)),
        };
        let event = match self.record_unbound_compute_event(
            session_occurrence,
            acceptance_epoch,
            &bound.retention,
            0,
        ) {
            Ok(event) => event,
            Err(error) => return Err((error, bound)),
        };
        let (packets, retention) = bound.into_parts();
        Ok(PreparedComputeDependencyTargetV1 {
            identity,
            retention,
            event,
            final_dispatch: packets.into_one(),
        })
    }

    /// Reauthenticates the sealed target and consumes its coupled dispatch
    /// only while building the exact dependency plan.
    #[allow(clippy::result_large_err)]
    pub(super) fn plan_dependency_target_v1(
        &self,
        target: PreparedComputeDependencyTargetV1,
        dependencies: &[AqlDependencySignalObservationV1],
    ) -> Result<
        PlannedComputeDependencyTargetV1,
        (
            ComputeDependencyTargetPlanErrorV1,
            PreparedComputeDependencyTargetV1,
        ),
    > {
        if let Err(error) = self.validate_dependency_target_v1(&target) {
            return Err((
                ComputeDependencyTargetPlanErrorV1::Completion(error),
                target,
            ));
        }
        let PreparedComputeDependencyTargetV1 {
            identity,
            retention,
            event,
            final_dispatch,
        } = target;
        match AqlPreparedDependencyDispatchV1::new(dependencies, final_dispatch) {
            Ok(plan) => Ok(PlannedComputeDependencyTargetV1 {
                identity,
                retention,
                event,
                plan,
            }),
            Err(failure) => {
                let error = failure.error();
                Err((
                    ComputeDependencyTargetPlanErrorV1::Plan(error),
                    PreparedComputeDependencyTargetV1 {
                        identity,
                        retention,
                        event,
                        final_dispatch: failure.into_final_dispatch(),
                    },
                ))
            }
        }
    }

    pub(super) fn validate_dependency_target_v1(
        &self,
        target: &PreparedComputeDependencyTargetV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        self.require_ready()?;
        self.validate_bound(&target.retention)?;
        let expected_identity = dependency_target_identity_v1(
            target.identity.session_occurrence,
            target.identity.acceptance_epoch,
            &target.retention,
        );
        if target.identity != expected_identity {
            return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
        }
        self.validate_unbound_dependency_target_event_v1(
            &target.event,
            target.identity.session_occurrence,
            target.identity.acceptance_epoch,
            &target.retention,
        )?;
        let expected_signal = self.dependency_target_signal_v1(&target.retention)?;
        if !target
            .final_dispatch
            .completion_signal_matches(expected_signal)
        {
            return Err(Gfx942CompletionErrorV1::InvalidArena(
                "dependency target completion signal",
            ));
        }
        Ok(())
    }

    fn dependency_target_signal_v1(
        &self,
        retention: &CompletionBatchRetentionV1<1>,
    ) -> Result<ObservedGpuAddressV1, Gfx942CompletionErrorV1> {
        let offset = u64::from(retention.slots[0].index)
            .checked_mul(AMD_SIGNAL_BYTES_V1 as u64)
            .ok_or(Gfx942CompletionErrorV1::InvalidArena(
                "dependency target signal offset",
            ))?;
        let raw =
            self.gpu_base
                .checked_add(offset)
                .ok_or(Gfx942CompletionErrorV1::InvalidArena(
                    "dependency target signal address",
                ))?;
        ObservedGpuAddressV1::new(raw)
            .map_err(|_| Gfx942CompletionErrorV1::InvalidArena("dependency target signal"))
    }

    pub(super) fn record_dependency_event_v1(
        &mut self,
        session_occurrence: u64,
        source_acceptance_epoch: u64,
        retention: &CompletionBatchRetentionV1<1>,
    ) -> Result<Gfx942ComputeEventOccurrenceV1, Gfx942CompletionErrorV1> {
        self.record_unbound_compute_event(session_occurrence, source_acceptance_epoch, retention, 0)
    }

    pub(super) fn record_dependency_event_at_v1<const N: usize>(
        &mut self,
        session_occurrence: u64,
        source_acceptance_epoch: u64,
        retention: &CompletionBatchRetentionV1<N>,
        batch_index: usize,
    ) -> Result<Gfx942ComputeEventOccurrenceV1, Gfx942CompletionErrorV1> {
        self.record_unbound_compute_event(
            session_occurrence,
            source_acceptance_epoch,
            retention,
            batch_index,
        )
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn bind_dependency_event_v1(
        &mut self,
        event: Gfx942ComputeEventOccurrenceV1,
        batch: &Gfx942CompletionBatchV1<1>,
    ) -> Result<
        Gfx942ComputeEventOccurrenceV1,
        (Gfx942CompletionErrorV1, Gfx942ComputeEventOccurrenceV1),
    > {
        self.bind_compute_event_after_publication(event, batch, 0)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn bind_dependency_event_at_v1<const N: usize>(
        &mut self,
        event: Gfx942ComputeEventOccurrenceV1,
        batch: &Gfx942CompletionBatchV1<N>,
        batch_index: usize,
    ) -> Result<
        Gfx942ComputeEventOccurrenceV1,
        (Gfx942CompletionErrorV1, Gfx942ComputeEventOccurrenceV1),
    > {
        self.bind_compute_event_after_publication(event, batch, batch_index)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn bind_dependency_event_batch_v1<const N: usize>(
        &mut self,
        events: Vec<Gfx942ComputeEventOccurrenceV1>,
        batch: &Gfx942CompletionBatchV1<N>,
    ) -> Result<
        Vec<Gfx942ComputeEventOccurrenceV1>,
        (Gfx942CompletionErrorV1, Vec<Gfx942ComputeEventOccurrenceV1>),
    > {
        completion_bind_dependency_event_batch_body!(completion_rust_expr, self, events, batch)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn retain_dependency_reader_v1(
        &mut self,
        event: Gfx942ComputeEventOccurrenceV1,
        session_occurrence: u64,
        dependent_acceptance_epoch: u64,
    ) -> Result<
        (
            Gfx942ComputeEventOccurrenceV1,
            Gfx942ComputeDependencyReaderLeaseV1,
        ),
        (Gfx942CompletionErrorV1, Gfx942ComputeEventOccurrenceV1),
    > {
        self.retain_compute_dependency_reader(event, session_occurrence, dependent_acceptance_epoch)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn retain_dependency_reader_batch_v1(
        &mut self,
        events: Vec<Gfx942ComputeEventOccurrenceV1>,
        session_occurrence: u64,
        dependent_acceptance_epoch: u64,
    ) -> Result<Gfx942ComputeDependencyReaderBatchV1, Gfx942ComputeEventBatchFailureV1> {
        self.retain_compute_dependency_reader_batch(
            events,
            session_occurrence,
            dependent_acceptance_epoch,
        )
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn release_dependency_reader_v1(
        &mut self,
        lease: Gfx942ComputeDependencyReaderLeaseV1,
    ) -> Result<
        Gfx942ComputeDependencyReaderReleaseObservationV1,
        (
            Gfx942CompletionErrorV1,
            Gfx942ComputeDependencyReaderLeaseV1,
        ),
    > {
        self.release_compute_dependency_reader(lease)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn release_dependency_reader_batch_v1(
        &mut self,
        retained: Gfx942ComputeDependencyReaderBatchV1,
    ) -> Result<Vec<Gfx942ComputeEventOccurrenceV1>, Gfx942ComputeDependencyReaderBatchFailureV1>
    {
        self.release_compute_dependency_reader_batch(retained)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn release_dependency_event_v1(
        &mut self,
        event: Gfx942ComputeEventOccurrenceV1,
    ) -> Result<
        Gfx942ComputeEventReleaseObservationV1,
        (Gfx942CompletionErrorV1, Gfx942ComputeEventOccurrenceV1),
    > {
        self.release_compute_event(event)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn release_dependency_event_batch_v1(
        &mut self,
        events: Vec<Gfx942ComputeEventOccurrenceV1>,
    ) -> Result<usize, (Gfx942CompletionErrorV1, Vec<Gfx942ComputeEventOccurrenceV1>)> {
        completion_release_dependency_event_batch_body!(completion_rust_expr, self, events)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn release_dependency_reader_event_batch_v1(
        &mut self,
        retained: Gfx942ComputeDependencyReaderBatchV1,
    ) -> Result<usize, Gfx942ComputeDependencyReaderBatchFailureV1> {
        self.release_compute_dependency_reader_event_batch(retained)
    }

    pub(super) fn dependency_source_identity_v1(
        &self,
        event: &Gfx942ComputeEventOccurrenceV1,
    ) -> Result<ComputeDependencyOccurrenceIdentityV1, Gfx942CompletionErrorV1> {
        self.project_compute_dependency_source_identity(event)
    }

    pub(super) fn native_dependency_signal_observation_v1(
        &self,
        lease: &Gfx942ComputeDependencyReaderLeaseV1,
    ) -> Result<AqlDependencySignalObservationV1, Gfx942CompletionErrorV1> {
        self.project_native_dependency_signal_observation(lease)
    }

    pub(super) fn matches_dependency_source_arena_v1(
        &self,
        queue: QueueKeyV1,
        signal_mapping: MemoryMappingKeyV1,
    ) -> bool {
        self.queue == queue && self.signal_mapping == signal_mapping
    }

    pub(super) fn matches_dependency_event_v1(
        &self,
        event: &Gfx942ComputeEventOccurrenceV1,
    ) -> bool {
        let (queue, signal_mapping) = event.arena_identity();
        self.matches_dependency_source_arena_v1(queue, signal_mapping)
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn cancel_prepared_dependency_target_v1(
        &mut self,
        target: PreparedComputeDependencyTargetV1,
    ) -> Result<(), Gfx942CompletionErrorV1> {
        self.validate_dependency_target_v1(&target)?;
        let PreparedComputeDependencyTargetV1 {
            retention, event, ..
        } = target;
        self.release_dependency_event_v1(event)
            .map_err(|(error, _event)| error)?;
        self.cancel_bound(retention)
    }

    pub(super) fn bound_dependency_target_identity_v1(
        &self,
        session_occurrence: u64,
        acceptance_epoch: u64,
        retention: &CompletionBatchRetentionV1<1>,
    ) -> Result<ComputeDependencyOccurrenceIdentityV1, Gfx942CompletionErrorV1> {
        self.validate_bound(retention)?;
        Ok(dependency_target_identity_v1(
            session_occurrence,
            acceptance_epoch,
            retention,
        ))
    }

    pub(super) fn validate_bound_dependency_target_custody_v1(
        &self,
        session_occurrence: u64,
        acceptance_epoch: u64,
        retention: &CompletionBatchRetentionV1<1>,
        event: &Gfx942ComputeEventOccurrenceV1,
    ) -> Result<ComputeDependencyOccurrenceIdentityV1, Gfx942CompletionErrorV1> {
        let identity = self.bound_dependency_target_identity_v1(
            session_occurrence,
            acceptance_epoch,
            retention,
        )?;
        self.validate_unbound_dependency_target_event_v1(
            event,
            session_occurrence,
            acceptance_epoch,
            retention,
        )?;
        Ok(identity)
    }

    pub(super) fn published_dependency_target_identity_v1(
        &self,
        session_occurrence: u64,
        acceptance_epoch: u64,
        batch: &Gfx942CompletionBatchV1<1>,
    ) -> Result<ComputeDependencyOccurrenceIdentityV1, Gfx942CompletionErrorV1> {
        self.validate_published(&batch.retention)?;
        Ok(dependency_target_identity_v1(
            session_occurrence,
            acceptance_epoch,
            &batch.retention,
        ))
    }

    pub(super) fn completed_dependency_target_identity_v1(
        &self,
        session_occurrence: u64,
        acceptance_epoch: u64,
        completed: &Gfx942CompletedBatchV1<1>,
    ) -> Result<ComputeDependencyOccurrenceIdentityV1, Gfx942CompletionErrorV1> {
        self.validate_completed(&completed.retention)?;
        Ok(dependency_target_identity_v1(
            session_occurrence,
            acceptance_epoch,
            &completed.retention,
        ))
    }
}

#[cfg(test)]
pub(super) fn substitute_dependency_target_component_for_test(
    mut first: PreparedComputeDependencyTargetV1,
    mut second: PreparedComputeDependencyTargetV1,
    substitution: ComputeDependencyTargetSubstitutionV1,
) -> (
    PreparedComputeDependencyTargetV1,
    PreparedComputeDependencyTargetV1,
) {
    match substitution {
        ComputeDependencyTargetSubstitutionV1::FinalDispatch => {
            core::mem::swap(&mut first.final_dispatch, &mut second.final_dispatch);
        }
        ComputeDependencyTargetSubstitutionV1::Retention => {
            core::mem::swap(&mut first.retention, &mut second.retention);
        }
        ComputeDependencyTargetSubstitutionV1::Event => {
            core::mem::swap(&mut first.event, &mut second.event);
        }
    }
    (first, second)
}

fn dependency_target_identity_v1(
    session_occurrence: u64,
    acceptance_epoch: u64,
    retention: &CompletionBatchRetentionV1<1>,
) -> ComputeDependencyOccurrenceIdentityV1 {
    let slot = retention.slots[0];
    let dispatch = retention.dispatches[0];
    ComputeDependencyOccurrenceIdentityV1 {
        session_occurrence,
        acceptance_epoch,
        batch_id: retention.batch_id,
        queue: retention.queue,
        signal_mapping: retention.signal_mapping,
        slot_index: slot.index,
        slot_generation: slot.generation,
        dispatch_generation: dispatch.dispatch_generation,
        packet_id: retention.last_packet_id,
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
    completion_packet_count_body!(completion_rust_expr, N)
}

#[cfg(test)]
#[path = "queue_completion/roster_projection_tests.rs"]
mod roster_projection_tests;

#[cfg(test)]
#[path = "queue_completion/tests.rs"]
mod tests;

#[path = "queue_completion/barrier_probe.rs"]
mod barrier_probe;

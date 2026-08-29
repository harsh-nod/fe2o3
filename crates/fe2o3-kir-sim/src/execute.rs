#![allow(
    clippy::result_large_err,
    reason = "exact simulator failures retain typed invocation, site, and bounded diagnostic data"
)]

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::mem::size_of;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Atomic, AtomicKind, Axis, BasicBlock, BinaryOp, BlockId, CastKind,
    CheckedBinaryOperator, ComparePredicate, Constant, Fence, Function, FunctionId, FunctionRole,
    IndexKind, IntrinsicKind, MemoryAccess, MemoryOrdering, Module, Operation, OperationKind,
    ScalarType, SynchronizationScope, Terminator, Type, UnaryOp, ValueDef, ValueId,
    VerifiedCanonicalKernelIrIdentityV7, WaveOperation, WaveOperationKind, WaveWidth,
    WorkgroupBarrier, WorkgroupMemoryExtent,
};

use crate::model::mask;
use crate::preflight::{supported_cast, supports_binary, supports_compare, supports_unary};
use crate::resident::{
    ResidentLedger, geometric_vec_bytes, hash_map_capacity_bytes,
    partitioned_bool_vec_storage_bytes, partitioned_geometric_vec_bytes, reserved_bool_vec_bytes,
    reserved_hash_map_bytes, reserved_vec_bytes,
};
use crate::schedule::{PreparedScheduleV1, SchedulePrepareErrorV1};
use crate::soft_float::{SoftFloatErrorV1, SoftFloatOperationV1};
use crate::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, EventPolicyV1,
    NoopSimulationDebugSinkV1, ScalarBitsV1, SharedBufferV1, SimulationArgumentV1,
    SimulationDebugAllocationV1, SimulationDebugBarrierActionV1, SimulationDebugBindingV1,
    SimulationDebugCaptureLimitsV1, SimulationDebugCheckpointPhaseV1, SimulationDebugCollectionV1,
    SimulationDebugFrameV1, SimulationDebugMemoryAccessV1, SimulationDebugRecordKindV1,
    SimulationDebugRecordV1, SimulationDebugScheduleV1, SimulationDebugSinkControlV1,
    SimulationDebugSinkV1, SimulationDebugSiteV1, SimulationDebugUnavailableReasonV1,
    SimulationDebugValueV1, SimulationInvocationV1, SimulationLimitsV1, SimulationPlanV1,
    SimulationPreflightErrorV1, SimulationRequestV1, SimulationScheduleCoverageV1,
    SimulationScheduleIdentityV1, SimulationScheduleRecordV1, SimulationScheduleReplayErrorV1,
    SimulationScheduleRequestV1, SimulationSiteV1, SimulationTargetV1,
};

/// Ephemeral execution event kind. This is an in-process adapter, not a durable trace schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationEventKindV1 {
    /// One logical invocation has started. It may later end as failed.
    InvocationBegin,
    /// One logical invocation has ended after either success or dynamic failure.
    InvocationEnd {
        outcome: SimulationExecutionOutcomeV1,
    },
    /// Control entered the named block after its arguments were bound.
    BlockEnter,
    /// An operation is about to be evaluated.
    OperationBegin,
    /// Operation evaluation has ended; this closes the lifecycle, not a success claim.
    OperationEnd {
        outcome: SimulationExecutionOutcomeV1,
    },
    /// A terminator is about to be evaluated after consuming one step.
    Terminator,
    /// The terminator selected this target after resolving its outgoing values.
    Branch { target: BlockId },
    /// A scalar load and initialization check completed successfully.
    MemoryRead {
        allocation: u64,
        offset: usize,
        bytes: usize,
    },
    /// A store is fully prepared; successful retention guarantees its infallible commit follows.
    MemoryWrite {
        allocation: u64,
        offset: usize,
        bytes: usize,
    },
    /// One indivisible integer atomic observation under the selected CPU schedule.
    MemoryAtomic {
        allocation: u64,
        offset: usize,
        bytes: usize,
        kind: AtomicKind,
        previous: Option<ScalarBitsV1>,
        committed: Option<ScalarBitsV1>,
        compare_exchange_success: Option<bool>,
        scope: SynchronizationScope,
        ordering: MemoryOrdering,
        failure_ordering: Option<MemoryOrdering>,
    },
    /// A scoped memory-order point. It does not synchronize invocation execution.
    MemoryFence {
        memory_scope: SynchronizationScope,
        ordering: MemoryOrdering,
        /// Bits 0 through 4 represent private, workgroup, global, constant,
        /// and generic address spaces, respectively.
        address_space_mask: u8,
    },
    /// An allocation that existed before the first invocation became observable.
    AllocationPreexisting {
        allocation: u64,
        address_space: AddressSpace,
        bytes: usize,
    },
    /// An allocation was created while evaluating the operation at this event's site.
    AllocationCreated {
        allocation: u64,
        address_space: AddressSpace,
        bytes: usize,
    },
    /// An allocation's semantic lifetime ended at this event's site.
    AllocationReleased { allocation: u64 },
    /// One live in-grid invocation arrived at a convergent workgroup barrier.
    WorkgroupBarrierArrive { phase: u64 },
    /// Every live in-grid invocation arrived and the workgroup phase was released.
    WorkgroupBarrierRelease { phase: u64, participants: u32 },
    /// Call arguments were resolved and control is about to enter the callee.
    Call {
        /// Canonical module-function ordinal of the callee.
        callee_ordinal: usize,
    },
    /// Return values were resolved and the function's private allocations were released.
    Return,
}

/// Whether an observed simulator lifecycle ended normally or through a dynamic failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationExecutionOutcomeV1 {
    Completed,
    Failed,
}

/// Nonfatal control returned by an ephemeral event sink.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationEventSinkControlV1 {
    /// The event was retained and delivery should continue.
    Continue,
    /// The event was retained; permanently stop callbacks and event accounting for this run.
    Stop,
    /// The event was not retained; permanently stop callbacks and event accounting for this run.
    DropAndStop,
}

/// One bounded ephemeral simulator event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationEventV1 {
    pub invocation: SimulationInvocationV1,
    pub site: SimulationEventSiteV1,
    pub kind: SimulationEventKindV1,
}

/// Allocation-free site carried by an ephemeral event callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationEventSiteV1 {
    /// Canonical module-function ordinal. The admitted module resolves it to an ID.
    pub function_ordinal: usize,
    pub block: BlockId,
    pub operation: Option<u32>,
}

/// Bounded result of cross-invocation global-memory conflict assessment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationConflictAssessmentV1 {
    NoConflictsObserved,
    ConflictsObserved {
        conflicting_bytes: u64,
        first: SimulationMemoryConflictV1,
    },
    Incomplete {
        conflicting_bytes: u64,
        first: Option<SimulationMemoryConflictV1>,
        /// The byte-access table reached its caller-supplied record bound.
        access_record_limit_reached: bool,
        /// A later access could depend on a representative evicted from the
        /// bounded per-byte read/write frontier.
        access_frontier_incomplete: bool,
        record_limit: usize,
    },
}

/// First observed byte-level conflicting access under the deterministic schedule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationMemoryConflictV1 {
    pub allocation: u64,
    pub offset: usize,
    pub earlier: SimulationInvocationV1,
    pub later: SimulationInvocationV1,
    pub earlier_site: SimulationSiteV1,
    pub later_site: SimulationSiteV1,
}

/// Bounded data-race assessment under one deterministic CPU schedule.
///
/// This is execution evidence, not a proof that another admitted interleaving is race-free.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationRaceAssessmentV1 {
    NoRacesObserved {
        first_ordered_conflict: Option<SimulationOrderedMemoryConflictV1>,
    },
    RacesObserved {
        racing_bytes: u64,
        first: SimulationDataRaceV1,
        first_ordered_conflict: Option<SimulationOrderedMemoryConflictV1>,
    },
    Incomplete {
        racing_bytes: u64,
        first: Option<SimulationDataRaceV1>,
        first_ordered_conflict: Option<SimulationOrderedMemoryConflictV1>,
        /// The byte-access table reached its caller-supplied record bound.
        access_record_limit_reached: bool,
        /// A later access could depend on a representative evicted from the
        /// bounded per-byte read/write frontier.
        access_frontier_incomplete: bool,
        /// Release/acquire atomic or fence HB may order an observed ordinary conflict.
        atomic_or_fence_happens_before_unmodeled: bool,
        record_limit: usize,
    },
}

/// First observed byte-level conflicting access not ordered by admitted synchronization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationDataRaceV1 {
    pub conflict: SimulationMemoryConflictV1,
    pub earlier_atomic: bool,
    pub later_atomic: bool,
}

/// Why one observed conflicting access pair is not a data race.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationHappensBeforeReasonV1 {
    AtomicSerialization,
    GlobalWorkgroupBarrier,
}

/// First conflicting pair that has an exact ordering in the simulator contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationOrderedMemoryConflictV1 {
    pub conflict: SimulationMemoryConflictV1,
    pub reason: SimulationHappensBeforeReasonV1,
}

/// Adapter for forwarding ephemeral simulator events to debugger or test infrastructure.
pub trait SimulationEventSinkV1 {
    /// Records one event atomically. An error means the event was not retained.
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1>;

    /// Records one event and optionally stops future observation without failing execution.
    ///
    /// Sinks that bound their own storage should return `Stop` with the last event they retain,
    /// or `DropAndStop` when the current event did not fit and was not retained.
    /// The default preserves the original always-continue contract.
    fn record_controlled(
        &mut self,
        event: &SimulationEventV1,
    ) -> Result<SimulationEventSinkControlV1, SimulationEventSinkErrorV1> {
        self.record(event)?;
        Ok(SimulationEventSinkControlV1::Continue)
    }
}

/// A fallible debugger/profiler event boundary failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationEventSinkErrorV1 {
    pub detail: String,
}

impl fmt::Display for SimulationEventSinkErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl Error for SimulationEventSinkErrorV1 {}

/// Event sink that discards every event.
#[derive(Default)]
pub struct NoopSimulationEventSinkV1;

impl SimulationEventSinkV1 for NoopSimulationEventSinkV1 {
    fn record(&mut self, _event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        Ok(())
    }
}

/// Completed deterministic execution and copied-back arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationExecutionV1 {
    identity: VerifiedCanonicalKernelIrIdentityV7,
    arguments: Vec<SimulationArgumentV1>,
    shared_buffers: Vec<SharedBufferV1>,
    invocations_executed: u64,
    workgroups_visited: u64,
    scheduled_slots_visited: u64,
    steps_executed: u64,
    events_emitted: u64,
    schedule: SimulationScheduleIdentityV1,
    schedule_transcript_identity: [u8; 32],
    schedule_coverage: SimulationScheduleCoverageV1,
    supplemental: Vec<SimulationSupplementalV1>,
    conflict_assessment: SimulationConflictAssessmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SimulationSupplementalV1 {
    schedule_records: Vec<SimulationScheduleRecordV1>,
    race_assessment: SimulationRaceAssessmentV1,
}

impl SimulationExecutionV1 {
    /// Returns the exact canonical KIR identity that was simulated.
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV7 {
        &self.identity
    }

    /// Returns copied-back scalar and buffer arguments in entry-ABI order.
    pub fn arguments(&self) -> &[SimulationArgumentV1] {
        &self.arguments
    }

    /// Returns a copied-back buffer by argument ordinal.
    pub fn buffer(&self, argument: usize) -> Option<&BufferArgumentV1> {
        match self.arguments.get(argument) {
            Some(SimulationArgumentV1::Buffer(buffer)) => Some(buffer),
            _ => None,
        }
    }

    /// Consumes the observation and returns all copied-back arguments.
    pub fn into_arguments(self) -> Vec<SimulationArgumentV1> {
        self.arguments
    }

    /// Consumes the observation and returns ABI arguments plus shared backings.
    pub fn into_outputs(self) -> (Vec<SimulationArgumentV1>, Vec<SharedBufferV1>) {
        (self.arguments, self.shared_buffers)
    }

    /// Returns copied-back named backing allocations.
    pub fn shared_buffers(&self) -> &[SharedBufferV1] {
        &self.shared_buffers
    }

    /// Returns one copied-back named backing allocation.
    pub fn shared_buffer(&self, id: BufferBackingIdV1) -> Option<&BufferArgumentV1> {
        self.shared_buffers
            .iter()
            .find(|shared| shared.id == id)
            .map(|shared| &shared.buffer)
    }

    pub const fn invocations_executed(&self) -> u64 {
        self.invocations_executed
    }

    pub const fn workgroups_visited(&self) -> u64 {
        self.workgroups_visited
    }

    pub const fn scheduled_slots_visited(&self) -> u64 {
        self.scheduled_slots_visited
    }

    pub const fn steps_executed(&self) -> u64 {
        self.steps_executed
    }

    pub const fn events_emitted(&self) -> u64 {
        self.events_emitted
    }

    pub const fn schedule(&self) -> SimulationScheduleIdentityV1 {
        self.schedule
    }

    /// Returns the exact identity of the realized semantic CPU ordering.
    pub const fn schedule_transcript_identity(&self) -> &[u8; 32] {
        &self.schedule_transcript_identity
    }

    /// Returns complete runnable-decision and cooperative-barrier coverage.
    pub const fn schedule_coverage(&self) -> SimulationScheduleCoverageV1 {
        self.schedule_coverage
    }

    /// Returns the bounded record when this run explicitly requested recording.
    pub fn schedule_record(&self) -> Option<&SimulationScheduleRecordV1> {
        self.supplemental
            .first()
            .and_then(|supplemental| supplemental.schedule_records.first())
    }

    pub const fn conflict_assessment(&self) -> &SimulationConflictAssessmentV1 {
        &self.conflict_assessment
    }

    /// Returns bounded race and happens-before evidence from this exact CPU schedule.
    pub fn race_assessment(&self) -> &SimulationRaceAssessmentV1 {
        &self
            .supplemental
            .first()
            .expect("successful execution retains one race assessment")
            .race_assessment
    }

    pub(crate) fn into_schedule_and_race(
        mut self,
    ) -> (
        Option<SimulationScheduleRecordV1>,
        SimulationRaceAssessmentV1,
    ) {
        let mut supplemental = self
            .supplemental
            .pop()
            .expect("successful execution retains supplemental observations");
        (
            supplemental.schedule_records.pop(),
            supplemental.race_assessment,
        )
    }

    /// CPU execution is an observation, never a proof or execution authority.
    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

/// Preflight or dynamic execution failure.
#[allow(
    clippy::large_enum_variant,
    reason = "dynamic failures retain exact primary and bounded secondary observation diagnostics"
)]
#[derive(Debug)]
pub enum SimulationErrorV1 {
    Preflight(SimulationPreflightErrorV1),
    Execution(SimulationExecutionErrorV1),
}

impl fmt::Display for SimulationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preflight(error) => error.fmt(formatter),
            Self::Execution(error) => error.fmt(formatter),
        }
    }
}

impl Error for SimulationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Preflight(error) => Some(error),
            Self::Execution(error) => Some(error),
        }
    }
}

/// Dynamic execution error with an exact invocation and KIR site when available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationExecutionErrorV1 {
    pub invocation: Option<SimulationInvocationV1>,
    pub site: Option<SimulationSiteV1>,
    pub kind: SimulationExecutionErrorKindV1,
    /// First failure encountered while truthfully closing observations after the primary error.
    pub observation_failure: Option<SimulationObservationFailureV1>,
}

/// A bounded secondary failure encountered while reporting or closing a primary failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationObservationFailureV1 {
    pub invocation: Option<SimulationInvocationV1>,
    pub site: Option<SimulationSiteV1>,
    pub kind: SimulationExecutionErrorKindV1,
}

impl fmt::Display for SimulationExecutionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "simulation execution failed: {:?}", self.kind)?;
        if let Some(site) = &self.site {
            write!(
                formatter,
                " at {}, {}, op {:?}",
                site.function, site.block, site.operation
            )?;
        }
        if let Some(invocation) = self.invocation {
            write!(formatter, " for global {:?}", invocation.global)?;
        }
        if let Some(observation) = &self.observation_failure {
            write!(
                formatter,
                "; observation closure also failed: {:?}",
                observation.kind
            )?;
        }
        Ok(())
    }
}

impl Error for SimulationExecutionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            SimulationExecutionErrorKindV1::EventSinkFailure(error) => Some(error),
            _ => self
                .observation_failure
                .as_ref()
                .and_then(|failure| match &failure.kind {
                    SimulationExecutionErrorKindV1::EventSinkFailure(error) => Some(error as _),
                    _ => None,
                }),
        }
    }
}

/// Closed dynamic failure classification for the first execution profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationExecutionErrorKindV1 {
    StepLimit {
        limit: u64,
    },
    EventLimit {
        limit: u64,
    },
    CallDepthLimit {
        limit: usize,
    },
    SsaValueLimit {
        limit: usize,
    },
    AllocationLimit {
        limit: usize,
    },
    AllocationBytesLimit {
        actual: usize,
        limit: usize,
    },
    TotalBytesLimit {
        actual: usize,
        limit: usize,
    },
    AllocationFailure,
    MissingFunction(FunctionId),
    MissingBody(FunctionId),
    UnknownBlock(BlockId),
    MissingTerminator(BlockId),
    UndefinedValue(ValueId),
    RuntimeType {
        value: Option<ValueId>,
        expected: &'static str,
    },
    ResultArity {
        expected: usize,
        actual: usize,
    },
    BlockArgumentArity {
        expected: usize,
        actual: usize,
    },
    UndefinedIntegerOperation(&'static str),
    IntegerOutOfRange,
    PointerOffsetOverflow,
    DanglingPointer {
        allocation: u64,
    },
    AddressSpaceMismatch,
    ReadOnlyWrite,
    MisalignedAccess {
        required: u32,
        offset: usize,
    },
    OutOfBounds {
        allocation: u64,
        offset: usize,
        bytes: usize,
        allocation_bytes: usize,
    },
    UninitializedRead {
        allocation: u64,
        offset: usize,
        bytes: usize,
    },
    WorkgroupUseBeforePublish {
        allocation: u64,
        offset: usize,
        bytes: usize,
    },
    DivergentWorkgroupBarrier(DivergentWorkgroupBarrierV1),
    MismatchedWorkgroupBarrier(MismatchedWorkgroupBarrierV1),
    IncompleteWave(IncompleteWaveV1),
    DivergentWave(DivergentWaveV1),
    MismatchedWave(MismatchedWaveV1),
    WaveShuffleSourceOutOfRange {
        source_lane: u32,
        tile_width: u32,
    },
    WorkgroupSchedulerNoProgress {
        phase: u64,
    },
    ScheduleDecisionLimit {
        actual: usize,
        limit: usize,
    },
    ScheduleResidentLimit {
        actual: usize,
        limit: usize,
    },
    ScheduleReplay(SimulationScheduleReplayErrorV1),
    ReachedUnreachable,
    InternalInvariant(&'static str),
    EventSinkFailure(SimulationEventSinkErrorV1),
}

impl SimulationExecutionErrorKindV1 {
    pub(crate) fn retained_heap_bytes(&self) -> usize {
        match self {
            Self::MissingFunction(function) | Self::MissingBody(function) => {
                function.retained_capacity_bytes()
            }
            Self::EventSinkFailure(error) => error.detail.capacity(),
            Self::StepLimit { .. }
            | Self::EventLimit { .. }
            | Self::CallDepthLimit { .. }
            | Self::SsaValueLimit { .. }
            | Self::AllocationLimit { .. }
            | Self::AllocationBytesLimit { .. }
            | Self::TotalBytesLimit { .. }
            | Self::AllocationFailure
            | Self::UnknownBlock(_)
            | Self::MissingTerminator(_)
            | Self::UndefinedValue(_)
            | Self::RuntimeType { .. }
            | Self::ResultArity { .. }
            | Self::BlockArgumentArity { .. }
            | Self::UndefinedIntegerOperation(_)
            | Self::IntegerOutOfRange
            | Self::PointerOffsetOverflow
            | Self::DanglingPointer { .. }
            | Self::AddressSpaceMismatch
            | Self::ReadOnlyWrite
            | Self::MisalignedAccess { .. }
            | Self::OutOfBounds { .. }
            | Self::UninitializedRead { .. }
            | Self::WorkgroupUseBeforePublish { .. }
            | Self::DivergentWorkgroupBarrier(_)
            | Self::MismatchedWorkgroupBarrier(_)
            | Self::IncompleteWave(_)
            | Self::DivergentWave(_)
            | Self::MismatchedWave(_)
            | Self::WaveShuffleSourceOutOfRange { .. }
            | Self::WorkgroupSchedulerNoProgress { .. }
            | Self::ScheduleDecisionLimit { .. }
            | Self::ScheduleResidentLimit { .. }
            | Self::ScheduleReplay(_)
            | Self::ReachedUnreachable
            | Self::InternalInvariant(_) => 0,
        }
    }
}

/// Which part of two same-phase workgroup barrier arrivals was incompatible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkgroupBarrierMismatchV1 {
    Site,
    Semantics,
    SiteAndSemantics,
}

/// Exact bounded detail for a participant exiting while peers wait at a barrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DivergentWorkgroupBarrierV1 {
    pub phase: u64,
    pub waiting: WorkgroupParticipantV1,
    pub exited: WorkgroupParticipantV1,
}

/// Exact bounded detail for incompatible same-phase barrier arrivals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MismatchedWorkgroupBarrierV1 {
    pub phase: u64,
    pub expected: SimulationEventSiteV1,
    pub mismatch: WorkgroupBarrierMismatchV1,
}

/// Exact active-mask evidence for a final logical wave that cannot satisfy the full-wave KIR contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IncompleteWaveV1 {
    pub width: WaveWidth,
    pub wave_in_workgroup: u64,
    pub active_mask: u64,
    pub required_mask: u64,
}

/// Exact bounded detail for a lane reaching a collective while one active peer does not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DivergentWaveV1 {
    pub width: WaveWidth,
    pub wave_in_workgroup: u64,
    pub nonparticipating: WorkgroupParticipantV1,
}

/// Exact bounded detail for active lanes reaching different collective operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MismatchedWaveV1 {
    pub width: WaveWidth,
    pub expected: SimulationEventSiteV1,
}

/// Allocation-free workgroup-local participant coordinate retained by diagnostics.
///
/// The enclosing execution error carries the exact workgroup and launch geometry,
/// so this local coordinate uniquely identifies the participant without duplicating
/// the full launch descriptor into every error variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkgroupParticipantV1 {
    pub local: [u32; 3],
}

impl From<SimulationInvocationV1> for WorkgroupParticipantV1 {
    fn from(invocation: SimulationInvocationV1) -> Self {
        Self {
            local: invocation.local,
        }
    }
}

impl AdmittedSimulationModuleV1 {
    /// Runs with no event delivery. Caller arguments remain unchanged on success or failure.
    pub fn simulate(
        &self,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
    ) -> Result<SimulationExecutionV1, SimulationErrorV1> {
        let mut sink = NoopSimulationEventSinkV1;
        self.simulate_with_sink(request, target, limits, &mut sink)
    }

    /// Runs with an explicit bounded schedule recording or exact replay policy.
    ///
    /// This controls deterministic CPU semantic ordering only. It does not model
    /// GPU scheduling, timing, performance, or physical wave execution.
    pub fn simulate_scheduled(
        &self,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        schedule: SimulationScheduleRequestV1<'_>,
    ) -> Result<SimulationExecutionV1, SimulationErrorV1> {
        self.simulate_scheduled_with_resident_offset(request, target, limits, schedule, 0)
    }

    pub(crate) fn simulate_scheduled_with_resident_offset(
        &self,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        schedule: SimulationScheduleRequestV1<'_>,
        resident_offset: usize,
    ) -> Result<SimulationExecutionV1, SimulationErrorV1> {
        let plan = self
            .preflight(request, target, limits)
            .map_err(SimulationErrorV1::Preflight)?;
        let mut event_sink = NoopSimulationEventSinkV1;
        let mut debug_sink = NoopSimulationDebugSinkV1;
        execute(
            self,
            request,
            ExecutionConfiguration {
                target,
                limits,
                policy: request.events,
                plan,
                debug_capture: SimulationDebugCaptureLimitsV1::disabled(),
                schedule: Some(schedule),
                resident_offset,
            },
            &mut event_sink,
            &mut debug_sink,
        )
        .map_err(SimulationErrorV1::Execution)
    }

    /// Runs with bounded ephemeral event delivery after complete preflight succeeds.
    pub fn simulate_with_sink(
        &self,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        sink: &mut impl SimulationEventSinkV1,
    ) -> Result<SimulationExecutionV1, SimulationErrorV1> {
        self.simulate_with_event_policy(request, target, limits, request.events, sink)
    }

    /// Runs with event delivery explicitly enabled without cloning or changing the request.
    ///
    /// This is the adapter boundary for bounded debugger and profiler collectors.
    pub fn simulate_observed_with_sink(
        &self,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        sink: &mut impl SimulationEventSinkV1,
    ) -> Result<SimulationExecutionV1, SimulationErrorV1> {
        self.simulate_with_event_policy(request, target, limits, EventPolicyV1::Enabled, sink)
    }

    /// Runs the ordinary simulator while recording an independent bounded debug observation.
    ///
    /// Debug records are derived from live interpreter state and do not alter the stable
    /// [`SimulationEventV1`] stream. Stopping the debug sink only stops debug delivery; the
    /// deterministic simulation continues to completion.
    pub fn simulate_debugged_with_sink(
        &self,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        capture: SimulationDebugCaptureLimitsV1,
        debug_sink: &mut impl SimulationDebugSinkV1,
    ) -> Result<SimulationExecutionV1, SimulationErrorV1> {
        let plan = self
            .preflight(request, target, limits)
            .map_err(SimulationErrorV1::Preflight)?;
        let mut event_sink = NoopSimulationEventSinkV1;
        execute(
            self,
            request,
            ExecutionConfiguration {
                target,
                limits,
                policy: request.events,
                plan,
                debug_capture: capture,
                schedule: None,
                resident_offset: 0,
            },
            &mut event_sink,
            debug_sink,
        )
        .map_err(SimulationErrorV1::Execution)
    }

    /// Runs a scheduled simulation with independent bounded debug snapshots.
    pub fn simulate_debugged_scheduled_with_sink(
        &self,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        schedule: SimulationScheduleRequestV1<'_>,
        capture: SimulationDebugCaptureLimitsV1,
        debug_sink: &mut impl SimulationDebugSinkV1,
    ) -> Result<SimulationExecutionV1, SimulationErrorV1> {
        let plan = self
            .preflight(request, target, limits)
            .map_err(SimulationErrorV1::Preflight)?;
        let mut event_sink = NoopSimulationEventSinkV1;
        execute(
            self,
            request,
            ExecutionConfiguration {
                target,
                limits,
                policy: request.events,
                plan,
                debug_capture: capture,
                schedule: Some(schedule),
                resident_offset: 0,
            },
            &mut event_sink,
            debug_sink,
        )
        .map_err(SimulationErrorV1::Execution)
    }

    fn simulate_with_event_policy(
        &self,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        policy: EventPolicyV1,
        sink: &mut impl SimulationEventSinkV1,
    ) -> Result<SimulationExecutionV1, SimulationErrorV1> {
        let plan = self
            .preflight(request, target, limits)
            .map_err(SimulationErrorV1::Preflight)?;
        let mut debug_sink = NoopSimulationDebugSinkV1;
        execute(
            self,
            request,
            ExecutionConfiguration {
                target,
                limits,
                policy,
                plan,
                debug_capture: SimulationDebugCaptureLimitsV1::disabled(),
                schedule: None,
                resident_offset: 0,
            },
            sink,
            &mut debug_sink,
        )
        .map_err(SimulationErrorV1::Execution)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuntimeValue {
    Scalar(ScalarBitsV1),
    Pointer(PointerValue),
    Slice(SliceValue),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PointerValue {
    allocation: u64,
    byte_offset: usize,
    element: ScalarType,
    address_space: AddressSpace,
    access: AccessMode,
    lower_bound: usize,
    upper_bound: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SliceValue {
    allocation: u64,
    elements: usize,
    element: ScalarType,
    address_space: AddressSpace,
    access: AccessMode,
    byte_offset: usize,
    byte_len: usize,
}

struct Allocation {
    address_space: AddressSpace,
    access: AccessMode,
    alignment: u32,
    bytes: Vec<u8>,
    initialized: Vec<bool>,
    workgroup_published: Vec<bool>,
    workgroup_writer: Vec<u64>,
}

struct WorkgroupAllocation {
    site: CompactSite,
    id: u64,
    element: ScalarType,
    bytes: usize,
    lifecycle_observed: bool,
}

struct PreparedStore<'a> {
    bytes: &'a mut [u8],
    initialized: &'a mut [bool],
}

impl PreparedStore<'_> {
    fn commit(self, value: ScalarBitsV1) {
        let raw = value.bits().to_le_bytes();
        self.bytes.copy_from_slice(&raw[..self.bytes.len()]);
        self.initialized.fill(true);
    }
}

struct Memory {
    allocations: HashMap<u64, Allocation>,
    argument_allocations: Vec<Option<u64>>,
    shared_allocations: HashMap<BufferBackingIdV1, u64>,
    next_allocation: u64,
    allocations_created: usize,
    live_bytes: usize,
}

impl Memory {
    fn new(
        argument_count: usize,
        shared_count: usize,
        limits: SimulationLimitsV1,
    ) -> Result<Self, SimulationExecutionErrorKindV1> {
        let mut allocations = HashMap::new();
        allocations
            .try_reserve(limits.max_allocations)
            .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
        let mut shared_allocations = HashMap::new();
        shared_allocations
            .try_reserve(shared_count)
            .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
        Ok(Self {
            allocations,
            argument_allocations: try_filled(argument_count, None)?,
            shared_allocations,
            next_allocation: 1,
            allocations_created: 0,
            live_bytes: 0,
        })
    }

    fn allocate(
        &mut self,
        address_space: AddressSpace,
        access: AccessMode,
        alignment: u32,
        bytes: Vec<u8>,
        initialized: Vec<bool>,
        limits: SimulationLimitsV1,
    ) -> Result<u64, SimulationExecutionErrorKindV1> {
        if bytes.len() != initialized.len() {
            return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "allocation initialization length",
            ));
        }
        self.validate_allocation(bytes.len(), limits)?;
        let (workgroup_published, workgroup_writer) = if address_space == AddressSpace::Workgroup {
            (
                try_filled(bytes.len(), false)?,
                try_filled(bytes.len(), 0_u64)?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let total = self.live_bytes.checked_add(bytes.len()).ok_or(
            SimulationExecutionErrorKindV1::TotalBytesLimit {
                actual: usize::MAX,
                limit: limits.max_total_bytes,
            },
        )?;
        let id = self.next_allocation;
        self.next_allocation = self.next_allocation.checked_add(1).ok_or(
            SimulationExecutionErrorKindV1::AllocationLimit {
                limit: limits.max_allocations,
            },
        )?;
        self.allocations_created += 1;
        self.live_bytes = total;
        self.allocations.insert(
            id,
            Allocation {
                address_space,
                access,
                alignment,
                bytes,
                initialized,
                workgroup_published,
                workgroup_writer,
            },
        );
        Ok(id)
    }

    fn validate_allocation(
        &self,
        bytes: usize,
        limits: SimulationLimitsV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        if self.allocations_created == limits.max_allocations {
            return Err(SimulationExecutionErrorKindV1::AllocationLimit {
                limit: limits.max_allocations,
            });
        }
        if bytes > limits.max_allocation_bytes {
            return Err(SimulationExecutionErrorKindV1::AllocationBytesLimit {
                actual: bytes,
                limit: limits.max_allocation_bytes,
            });
        }
        let total = self.live_bytes.checked_add(bytes).ok_or(
            SimulationExecutionErrorKindV1::TotalBytesLimit {
                actual: usize::MAX,
                limit: limits.max_total_bytes,
            },
        )?;
        if total > limits.max_total_bytes {
            return Err(SimulationExecutionErrorKindV1::TotalBytesLimit {
                actual: total,
                limit: limits.max_total_bytes,
            });
        }
        Ok(())
    }

    fn release_one(&mut self, id: u64) -> Result<bool, SimulationExecutionErrorKindV1> {
        let Some(allocation) = self.allocations.remove(&id) else {
            return Ok(false);
        };
        self.live_bytes = self.live_bytes.checked_sub(allocation.bytes.len()).ok_or(
            SimulationExecutionErrorKindV1::InternalInvariant(
                "released allocation live-byte accounting",
            ),
        )?;
        Ok(true)
    }

    fn load(
        &self,
        pointer: &PointerValue,
        access: MemoryAccess,
        target: SimulationTargetV1,
        invocation: SimulationInvocationV1,
    ) -> Result<ScalarBitsV1, SimulationExecutionErrorKindV1> {
        let allocation = self.allocation(pointer)?;
        let width = target.scalar_bytes(pointer.element).ok_or(
            SimulationExecutionErrorKindV1::InternalInvariant("preflighted load element"),
        )?;
        validate_access(allocation, pointer, access, width, false)?;
        let end = pointer
            .byte_offset
            .checked_add(width)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        if allocation.initialized[pointer.byte_offset..end]
            .iter()
            .any(|initialized| !initialized)
        {
            return Err(SimulationExecutionErrorKindV1::UninitializedRead {
                allocation: pointer.allocation,
                offset: pointer.byte_offset,
                bytes: width,
            });
        }
        if pointer.address_space == AddressSpace::Workgroup {
            let writer = invocation_local_ordinal(invocation)
                .and_then(|ordinal| ordinal.checked_add(1))
                .ok_or(SimulationExecutionErrorKindV1::InternalInvariant(
                    "workgroup invocation ordinal",
                ))?;
            if allocation.workgroup_published[pointer.byte_offset..end]
                .iter()
                .zip(&allocation.workgroup_writer[pointer.byte_offset..end])
                .any(|(published, owner)| !published && *owner != writer)
            {
                return Err(SimulationExecutionErrorKindV1::WorkgroupUseBeforePublish {
                    allocation: pointer.allocation,
                    offset: pointer.byte_offset,
                    bytes: width,
                });
            }
        }
        let mut raw = [0_u8; 16];
        raw[..width].copy_from_slice(&allocation.bytes[pointer.byte_offset..end]);
        ScalarBitsV1::new(pointer.element, u128::from_le_bytes(raw), target)
            .map_err(|_| SimulationExecutionErrorKindV1::InternalInvariant("loaded scalar bits"))
    }

    fn validate_store(
        &self,
        pointer: &PointerValue,
        access: MemoryAccess,
        value: ScalarBitsV1,
        target: SimulationTargetV1,
    ) -> Result<usize, SimulationExecutionErrorKindV1> {
        if value.ty() != pointer.element {
            return Err(SimulationExecutionErrorKindV1::RuntimeType {
                value: None,
                expected: "pointer element scalar",
            });
        }
        let width = target.scalar_bytes(pointer.element).ok_or(
            SimulationExecutionErrorKindV1::InternalInvariant("preflighted store element"),
        )?;
        let allocation = self.allocation(pointer)?;
        validate_access(allocation, pointer, access, width, true)?;
        Ok(width)
    }

    fn prepare_store(
        &mut self,
        pointer: &PointerValue,
        width: usize,
    ) -> Result<PreparedStore<'_>, SimulationExecutionErrorKindV1> {
        let allocation = self.allocations.get_mut(&pointer.allocation).ok_or(
            SimulationExecutionErrorKindV1::InternalInvariant("validated allocation remained live"),
        )?;
        let end = pointer
            .byte_offset
            .checked_add(width)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        let bytes = allocation.bytes.get_mut(pointer.byte_offset..end).ok_or(
            SimulationExecutionErrorKindV1::InternalInvariant(
                "validated store byte range remained live",
            ),
        )?;
        let initialized = allocation
            .initialized
            .get_mut(pointer.byte_offset..end)
            .ok_or(SimulationExecutionErrorKindV1::InternalInvariant(
                "validated store initialization range remained live",
            ))?;
        Ok(PreparedStore { bytes, initialized })
    }

    fn mark_workgroup_store(
        &mut self,
        pointer: &PointerValue,
        width: usize,
        invocation: SimulationInvocationV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        if pointer.address_space != AddressSpace::Workgroup {
            return Ok(());
        }
        let allocation = self.allocations.get_mut(&pointer.allocation).ok_or(
            SimulationExecutionErrorKindV1::DanglingPointer {
                allocation: pointer.allocation,
            },
        )?;
        let end = pointer
            .byte_offset
            .checked_add(width)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        allocation.workgroup_published[pointer.byte_offset..end].fill(false);
        let writer = invocation_local_ordinal(invocation)
            .and_then(|ordinal| ordinal.checked_add(1))
            .ok_or(SimulationExecutionErrorKindV1::InternalInvariant(
                "workgroup invocation ordinal",
            ))?;
        allocation.workgroup_writer[pointer.byte_offset..end].fill(writer);
        Ok(())
    }

    fn mark_workgroup_atomic(
        &mut self,
        pointer: &PointerValue,
        width: usize,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        if pointer.address_space != AddressSpace::Workgroup {
            return Ok(());
        }
        let allocation = self.allocations.get_mut(&pointer.allocation).ok_or(
            SimulationExecutionErrorKindV1::DanglingPointer {
                allocation: pointer.allocation,
            },
        )?;
        let end = pointer
            .byte_offset
            .checked_add(width)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        allocation.workgroup_published[pointer.byte_offset..end].fill(true);
        allocation.workgroup_writer[pointer.byte_offset..end].fill(0);
        Ok(())
    }

    fn publish_workgroup(&mut self) {
        for allocation in self
            .allocations
            .values_mut()
            .filter(|allocation| allocation.address_space == AddressSpace::Workgroup)
        {
            for (published, initialized) in allocation
                .workgroup_published
                .iter_mut()
                .zip(&allocation.initialized)
            {
                *published |= *initialized;
            }
        }
    }

    fn allocation(
        &self,
        pointer: &PointerValue,
    ) -> Result<&Allocation, SimulationExecutionErrorKindV1> {
        let allocation = self.allocations.get(&pointer.allocation).ok_or(
            SimulationExecutionErrorKindV1::DanglingPointer {
                allocation: pointer.allocation,
            },
        )?;
        if allocation.address_space != pointer.address_space {
            return Err(SimulationExecutionErrorKindV1::AddressSpaceMismatch);
        }
        Ok(allocation)
    }
}

fn invocation_local_ordinal(invocation: SimulationInvocationV1) -> Option<u64> {
    let x = u64::from(invocation.local[0]);
    let y = u64::from(invocation.local[1]);
    let z = u64::from(invocation.local[2]);
    let width = u64::from(invocation.workgroup_size[0]);
    let plane = width.checked_mul(u64::from(invocation.workgroup_size[1]))?;
    x.checked_add(y.checked_mul(width)?)?
        .checked_add(z.checked_mul(plane)?)
}

fn validate_access(
    allocation: &Allocation,
    pointer: &PointerValue,
    access: MemoryAccess,
    width: usize,
    write: bool,
) -> Result<(), SimulationExecutionErrorKindV1> {
    if access.address_space != pointer.address_space {
        return Err(SimulationExecutionErrorKindV1::AddressSpaceMismatch);
    }
    if write
        && (pointer.access != AccessMode::ReadWrite
            || allocation.access != AccessMode::ReadWrite
            || pointer.address_space == AddressSpace::Constant)
    {
        return Err(SimulationExecutionErrorKindV1::ReadOnlyWrite);
    }
    if access.alignment == 0
        || allocation.alignment < access.alignment
        || !pointer
            .byte_offset
            .is_multiple_of(access.alignment as usize)
    {
        return Err(SimulationExecutionErrorKindV1::MisalignedAccess {
            required: access.alignment,
            offset: pointer.byte_offset,
        });
    }
    let end = pointer.byte_offset.checked_add(width).ok_or(
        SimulationExecutionErrorKindV1::OutOfBounds {
            allocation: pointer.allocation,
            offset: pointer.byte_offset,
            bytes: width,
            allocation_bytes: allocation.bytes.len(),
        },
    )?;
    if pointer.byte_offset < pointer.lower_bound
        || end > pointer.upper_bound
        || end > allocation.bytes.len()
    {
        return Err(SimulationExecutionErrorKindV1::OutOfBounds {
            allocation: pointer.allocation,
            offset: pointer.byte_offset,
            bytes: width,
            allocation_bytes: allocation.bytes.len(),
        });
    }
    Ok(())
}

struct Engine<'a, S> {
    module: &'a fe2o3_kernel_ir::Module,
    function_module_indices: Vec<usize>,
    block_indices: Vec<HashMap<BlockId, usize>>,
    function_ssa_values: Vec<usize>,
    call_targets: Vec<Vec<Vec<CallTarget>>>,
    switch_targets: Vec<Vec<SwitchLookup>>,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    policy: EventPolicyV1,
    memory: Memory,
    sink: &'a mut S,
    debug_capture: SimulationDebugCaptureLimitsV1,
    debug_sink: &'a mut dyn SimulationDebugSinkV1,
    debug_records: u64,
    debug_delivery_stopped: bool,
    schedule_identity: SimulationScheduleIdentityV1,
    schedule_decision: u64,
    steps: u64,
    events: u64,
    reserved_event_closures: u64,
    event_delivery_stopped: bool,
    invocation: Option<SimulationInvocationV1>,
    accesses: HashMap<(u64, usize), AccessFrontier>,
    conflicting_bytes: u64,
    first_conflict: Option<SimulationMemoryConflictV1>,
    conflict_incomplete: bool,
    workgroup_happens_before_epoch: u64,
    unmodeled_atomic_or_fence_happens_before: bool,
    race_trackers: Vec<RaceTracker>,
    workgroup_allocations: Vec<WorkgroupAllocation>,
}

struct RaceTracker {
    racing_bytes: u64,
    first_race: Option<SimulationDataRaceV1>,
    first_ordered_conflict: Option<SimulationOrderedMemoryConflictV1>,
}

#[derive(Clone, Copy, Default)]
struct AccessFrontier {
    write: Option<LastAccess>,
    displaced_write: Option<LastAccess>,
    read: Option<LastAccess>,
    displaced_read: Option<LastAccess>,
    conflicted: bool,
    raced: bool,
    incomplete: bool,
    lost_write: bool,
    lost_writes_all_atomic: bool,
    lost_read: bool,
    lost_reads_all_atomic: bool,
}

#[derive(Clone, Copy)]
struct LastAccess {
    invocation: SimulationInvocationV1,
    site: CompactSite,
    atomic: bool,
    happens_before_epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct CompactSite {
    function: usize,
    block: BlockId,
    operation: Option<u32>,
}

enum SwitchLookup {
    None,
    Index(HashMap<u128, usize>),
    Integer(HashMap<(ScalarType, u128), usize>),
}

#[derive(Clone, Copy)]
enum CallTarget {
    NotCall,
    Internal(usize),
    Float(SoftFloatOperationV1),
}

fn debug_value(value: &RuntimeValue) -> SimulationDebugValueV1 {
    match value {
        RuntimeValue::Scalar(value) => SimulationDebugValueV1::Scalar(*value),
        RuntimeValue::Pointer(value) => SimulationDebugValueV1::Pointer {
            allocation: value.allocation,
            byte_offset: value.byte_offset,
            element: value.element,
            address_space: value.address_space,
            access: value.access,
            lower_bound: value.lower_bound,
            upper_bound: value.upper_bound,
        },
        RuntimeValue::Slice(value) => SimulationDebugValueV1::Slice {
            allocation: value.allocation,
            elements: value.elements,
            element: value.element,
            address_space: value.address_space,
            access: value.access,
            byte_offset: value.byte_offset,
            byte_len: value.byte_len,
        },
    }
}

fn capture_debug_stack(
    frames: &[RuntimeFrame<'_>],
    function_module_indices: &[usize],
    limits: SimulationDebugCaptureLimitsV1,
) -> SimulationDebugCollectionV1<SimulationDebugFrameV1> {
    if frames.len() > limits.max_frames_per_checkpoint() {
        return SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::FrameLimit,
            required: u64::try_from(frames.len()).unwrap_or(u64::MAX),
        };
    }
    let value_count = frames.iter().try_fold(0_usize, |count, frame| {
        count.checked_add(frame.values.len())
    });
    let Some(value_count) = value_count else {
        return SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::ValueLimit,
            required: u64::MAX,
        };
    };
    if value_count > limits.max_values_per_checkpoint() {
        return SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::ValueLimit,
            required: u64::try_from(value_count).unwrap_or(u64::MAX),
        };
    }
    let mut captured = Vec::new();
    if captured.try_reserve_exact(frames.len()).is_err() {
        return SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::AllocationFailure,
            required: u64::try_from(frames.len()).unwrap_or(u64::MAX),
        };
    }
    for (depth, frame) in frames.iter().enumerate() {
        let mut ordered = Vec::new();
        if ordered.try_reserve_exact(frame.values.len()).is_err() {
            return SimulationDebugCollectionV1::Unavailable {
                reason: SimulationDebugUnavailableReasonV1::AllocationFailure,
                required: u64::try_from(value_count).unwrap_or(u64::MAX),
            };
        }
        ordered.extend(frame.values.iter());
        ordered.sort_unstable_by_key(|(value, _)| **value);
        let mut values = Vec::new();
        if values.try_reserve_exact(ordered.len()).is_err() {
            return SimulationDebugCollectionV1::Unavailable {
                reason: SimulationDebugUnavailableReasonV1::AllocationFailure,
                required: u64::try_from(value_count).unwrap_or(u64::MAX),
            };
        }
        values.extend(
            ordered
                .into_iter()
                .map(|(value, observed)| SimulationDebugBindingV1 {
                    value: *value,
                    observed: debug_value(observed),
                }),
        );
        let Some(function_ordinal) = function_module_indices.get(frame.function_index).copied()
        else {
            return SimulationDebugCollectionV1::Unavailable {
                reason: SimulationDebugUnavailableReasonV1::NotCaptured,
                required: 0,
            };
        };
        captured.push(SimulationDebugFrameV1 {
            depth: u32::try_from(depth).unwrap_or(u32::MAX),
            function_ordinal,
            block: frame.current,
            next_operation: frame
                .function
                .body
                .as_ref()
                .and_then(|body| body.blocks.get(frame.current_index))
                .and_then(|block| block.operations.get(frame.operation))
                .and_then(|_| u32::try_from(frame.operation).ok()),
            values: SimulationDebugCollectionV1::Captured(values),
        });
    }
    SimulationDebugCollectionV1::Captured(captured)
}

fn capture_debug_memory(
    memory: &Memory,
    limits: SimulationDebugCaptureLimitsV1,
) -> SimulationDebugCollectionV1<SimulationDebugAllocationV1> {
    if memory.allocations.len() > limits.max_allocations_per_checkpoint() {
        return SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::AllocationLimit,
            required: u64::try_from(memory.allocations.len()).unwrap_or(u64::MAX),
        };
    }
    let Some(byte_count) = memory
        .allocations
        .values()
        .try_fold(0_usize, |count, allocation| {
            count
                .checked_add(allocation.bytes.len())?
                .checked_add(allocation.initialized.len())
        })
    else {
        return SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::MemoryByteLimit,
            required: u64::MAX,
        };
    };
    if byte_count > limits.max_memory_bytes_per_checkpoint() {
        return SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::MemoryByteLimit,
            required: u64::try_from(byte_count).unwrap_or(u64::MAX),
        };
    }
    let mut ordered = Vec::new();
    if ordered.try_reserve_exact(memory.allocations.len()).is_err() {
        return SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::AllocationFailure,
            required: u64::try_from(memory.allocations.len()).unwrap_or(u64::MAX),
        };
    }
    ordered.extend(memory.allocations.iter());
    ordered.sort_unstable_by_key(|(id, _)| **id);
    let mut captured = Vec::new();
    if captured.try_reserve_exact(ordered.len()).is_err() {
        return SimulationDebugCollectionV1::Unavailable {
            reason: SimulationDebugUnavailableReasonV1::AllocationFailure,
            required: u64::try_from(ordered.len()).unwrap_or(u64::MAX),
        };
    }
    for (id, allocation) in ordered {
        let mut bytes = Vec::new();
        let mut initialized = Vec::new();
        if bytes.try_reserve_exact(allocation.bytes.len()).is_err()
            || initialized
                .try_reserve_exact(allocation.initialized.len())
                .is_err()
        {
            return SimulationDebugCollectionV1::Unavailable {
                reason: SimulationDebugUnavailableReasonV1::AllocationFailure,
                required: u64::try_from(byte_count).unwrap_or(u64::MAX),
            };
        }
        bytes.extend_from_slice(&allocation.bytes);
        initialized.extend(allocation.initialized.iter().copied());
        captured.push(SimulationDebugAllocationV1 {
            allocation: *id,
            address_space: allocation.address_space,
            access: allocation.access,
            alignment: allocation.alignment,
            bytes,
            initialized,
        });
    }
    SimulationDebugCollectionV1::Captured(captured)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn conservative_execution_resident_bytes(
    admitted_resident_bytes: usize,
    request: &SimulationRequestV1,
    limits: SimulationLimitsV1,
    reachable_ssa_values: usize,
    plan_identity_bytes: usize,
    reachable_indices_capacity: usize,
    execution_index_resident_bytes: usize,
    maximum_reachable_identifier_bytes: usize,
    workgroup_participants: usize,
    workgroup_allocation_sites: usize,
    workgroup_static_bytes: usize,
) -> Option<usize> {
    let arguments = request.arguments.len();
    let shared = request.shared_buffers.len();
    let mut resident = ResidentLedger::new(admitted_resident_bytes);
    resident.add_bytes(size_of::<Engine<'static, NoopSimulationEventSinkV1>>())?;
    resident.add_bytes(reserved_vec_bytes::<RaceTracker>(1)?)?;
    resident.add_bytes(size_of::<SimulationRequestV1>())?;
    resident.add_bytes(size_of::<SimulationPlanV1>())?;
    resident.add_bytes(size_of::<SimulationExecutionV1>())?;
    resident.add_bytes(reserved_vec_bytes::<SimulationSupplementalV1>(1)?)?;
    resident.add_bytes(request.kernel.retained_capacity_bytes())?;
    resident.add_bytes(plan_identity_bytes)?;
    resident.add_vec::<usize>(reachable_indices_capacity)?;
    resident.add_bytes(execution_index_resident_bytes)?;
    // At successful return the live engine can own first conflict, first race,
    // and first ordered-conflict evidence while the returned conflict and race
    // assessments own clones of all four sites: twelve function identities.
    // Dynamic primary plus bounded secondary errors require no more clones.
    resident.add_product(maximum_reachable_identifier_bytes, 12)?;

    // The borrowed request and completed result coexist with simulated memory at copy-back.
    resident.add_product(
        request.arguments.capacity(),
        size_of::<SimulationArgumentV1>(),
    )?;
    resident.add_product(
        request.shared_buffers.capacity(),
        size_of::<SharedBufferV1>(),
    )?;
    for argument in &request.arguments {
        if let SimulationArgumentV1::Buffer(buffer) = argument {
            resident.add_bytes(buffer.retained_payload_capacity_bytes()?)?;
        }
    }
    for backing in &request.shared_buffers {
        resident.add_bytes(backing.buffer.retained_payload_capacity_bytes()?)?;
    }
    resident.add_bytes(reserved_vec_bytes::<SimulationArgumentV1>(arguments)?)?;
    resident.add_bytes(reserved_vec_bytes::<SharedBufferV1>(shared)?)?;
    for argument in &request.arguments {
        if let SimulationArgumentV1::Buffer(buffer) = argument {
            resident.add_bytes(reserved_vec_bytes::<u8>(buffer.bytes().len())?)?;
            resident.add_bytes(reserved_bool_vec_bytes(buffer.initialized().len())?)?;
        }
    }
    for backing in &request.shared_buffers {
        resident.add_bytes(reserved_vec_bytes::<u8>(backing.buffer.bytes().len())?)?;
        resident.add_bytes(reserved_bool_vec_bytes(backing.buffer.initialized().len())?)?;
    }

    // Allocation payloads use exact reservation on the pinned toolchain. Rust's
    // specialized `Vec<bool>` reports capacity in bits, not bytes.
    resident.add_bytes(limits.max_total_bytes)?;
    resident.add_bytes(partitioned_bool_vec_storage_bytes(
        limits.max_total_bytes,
        limits.max_allocations,
    )?)?;
    resident.add_bytes(reserved_hash_map_bytes::<u64, Allocation>(
        limits.max_allocations,
    )?)?;
    resident.add_bytes(reserved_hash_map_bytes::<BufferBackingIdV1, u64>(shared)?)?;
    resident.add_bytes(reserved_vec_bytes::<Option<u64>>(arguments)?)?;

    // The scheduler retains parameters, frame storage, SSA tables and branch/call temporaries.
    resident.add_product(reserved_vec_bytes::<RuntimeValue>(arguments)?, 2)?;
    resident.add_bytes(reserved_vec_bytes::<InvocationMachine<'static>>(
        workgroup_participants,
    )?)?;
    resident.add_bytes(reserved_vec_bytes::<(usize, ScalarBitsV1)>(
        workgroup_participants,
    )?)?;
    resident.add_product(
        workgroup_participants,
        geometric_vec_bytes::<RuntimeFrame<'static>>(limits.max_call_depth)?,
    )?;
    // InvocationMachine::new owns the next frame before it is moved into one reserved stack.
    resident.add_bytes(size_of::<RuntimeFrame<'static>>())?;
    let values_per_frame = reserved_hash_map_bytes::<ValueId, RuntimeValue>(reachable_ssa_values)?;
    resident.add_product(
        workgroup_participants.checked_mul(limits.max_call_depth)?,
        values_per_frame,
    )?;
    let incoming_per_frame = reserved_vec_bytes::<RuntimeValue>(reachable_ssa_values)?;
    resident.add_product(
        workgroup_participants.checked_mul(limits.max_call_depth)?,
        incoming_per_frame,
    )?;
    resident.add_product(reserved_vec_bytes::<RuntimeValue>(reachable_ssa_values)?, 2)?;
    resident.add_bytes(partitioned_geometric_vec_bytes::<FrameAllocation>(
        limits.max_allocations,
        workgroup_participants.checked_mul(limits.max_call_depth)?,
    )?)?;
    resident.add_bytes(reserved_vec_bytes::<WorkgroupAllocation>(
        workgroup_allocation_sites,
    )?)?;
    resident.add_bytes(reserved_bool_vec_bytes(workgroup_static_bytes)?)?;
    resident.add_product(workgroup_static_bytes, size_of::<u64>())?;

    resident.add_bytes(reserved_hash_map_bytes::<(u64, usize), AccessFrontier>(
        limits.max_memory_access_records,
    )?)?;
    Some(resident.bytes())
}

#[cfg(test)]
mod execution_resident_tests {
    use super::*;

    #[test]
    fn successful_assessment_accounts_twelve_live_function_identity_allocations() {
        let request = SimulationRequestV1::new("resident", [1, 1, 1], [1, 1, 1], vec![]);
        let limits = SimulationLimitsV1 {
            max_call_depth: 1,
            max_ssa_values: 1,
            max_allocations: 1,
            max_allocation_bytes: 1,
            max_total_bytes: 1,
            max_memory_access_records: 1,
            ..SimulationLimitsV1::default()
        };
        let accounted = |identifier_bytes| {
            conservative_execution_resident_bytes(
                0,
                &request,
                limits,
                0,
                0,
                0,
                0,
                identifier_bytes,
                1,
                0,
                0,
            )
            .expect("bounded resident accounting")
        };
        assert_eq!(accounted(257) - accounted(0), 12 * 257);
    }
}

impl<S: SimulationEventSinkV1> Engine<'_, S> {
    fn fail(&self, kind: SimulationExecutionErrorKindV1) -> SimulationExecutionErrorV1 {
        SimulationExecutionErrorV1 {
            invocation: self.invocation,
            site: None,
            kind,
            observation_failure: None,
        }
    }

    fn at(
        &self,
        site: CompactSite,
        kind: SimulationExecutionErrorKindV1,
    ) -> SimulationExecutionErrorV1 {
        SimulationExecutionErrorV1 {
            invocation: self.invocation,
            site: Some(self.materialize_site(site)),
            kind,
            observation_failure: None,
        }
    }

    fn materialize_site(&self, site: CompactSite) -> SimulationSiteV1 {
        let module_index = self.function_module_indices[site.function];
        SimulationSiteV1 {
            function: self.module.functions[module_index].id.clone(),
            block: site.block,
            operation: site.operation,
        }
    }

    fn materialize_event_site(&self, site: CompactSite) -> SimulationEventSiteV1 {
        SimulationEventSiteV1 {
            function_ordinal: self.function_module_indices[site.function],
            block: site.block,
            operation: site.operation,
        }
    }

    fn debug_checkpoint(
        &mut self,
        frames: &[RuntimeFrame<'_>],
        site: CompactSite,
        phase: SimulationDebugCheckpointPhaseV1,
    ) {
        if self.debug_delivery_stopped {
            return;
        }
        let stack = capture_debug_stack(frames, &self.function_module_indices, self.debug_capture);
        let memory = capture_debug_memory(&self.memory, self.debug_capture);
        self.deliver_debug(
            site,
            SimulationDebugRecordKindV1::Checkpoint {
                phase,
                stack,
                memory,
            },
        );
    }

    fn debug_memory(
        &mut self,
        site: CompactSite,
        access: SimulationDebugMemoryAccessV1,
        pointer: &PointerValue,
        byte_len: usize,
        value: ScalarBitsV1,
    ) {
        self.deliver_debug(
            site,
            SimulationDebugRecordKindV1::Memory {
                access,
                allocation: pointer.allocation,
                byte_offset: pointer.byte_offset,
                byte_len,
                address_space: pointer.address_space,
                value: SimulationDebugValueV1::Scalar(value),
            },
        );
    }

    fn debug_barrier(
        &mut self,
        site: CompactSite,
        action: SimulationDebugBarrierActionV1,
        phase: u64,
        participants: u32,
    ) {
        self.deliver_debug(
            site,
            SimulationDebugRecordKindV1::WorkgroupBarrier {
                action,
                phase,
                participants,
            },
        );
    }

    fn debug_fence(&mut self, site: CompactSite, fence: &Fence) {
        self.deliver_debug(
            site,
            SimulationDebugRecordKindV1::Fence {
                memory_scope: fence.memory_scope,
                ordering: fence.semantics.ordering,
                address_space_mask: address_space_mask(&fence.semantics.address_spaces),
            },
        );
    }

    fn deliver_debug(&mut self, site: CompactSite, kind: SimulationDebugRecordKindV1) {
        if self.debug_delivery_stopped {
            return;
        }
        let (Some(invocation), Some(operation)) = (self.invocation, site.operation) else {
            return;
        };
        let module_index = self.function_module_indices[site.function];
        let record = SimulationDebugRecordV1 {
            ordinal: self.debug_records,
            schedule: SimulationDebugScheduleV1 {
                identity: self.schedule_identity,
                decision_ordinal: self.schedule_decision,
            },
            invocation,
            site: SimulationDebugSiteV1 {
                function_ordinal: module_index,
                block: site.block,
                operation,
            },
            kind,
        };
        match self.debug_sink.record(record) {
            SimulationDebugSinkControlV1::Continue => {
                if let Some(next) = self.debug_records.checked_add(1) {
                    self.debug_records = next;
                } else {
                    self.debug_delivery_stopped = true;
                }
            }
            SimulationDebugSinkControlV1::Stop => {
                self.debug_records = self.debug_records.saturating_add(1);
                self.debug_delivery_stopped = true;
            }
            SimulationDebugSinkControlV1::DropAndStop => {
                self.debug_delivery_stopped = true;
            }
        }
    }

    fn step(&mut self, site: &CompactSite) -> Result<(), SimulationExecutionErrorV1> {
        self.charge_steps(site, 1)
    }

    fn charge_steps(
        &mut self,
        site: &CompactSite,
        count: usize,
    ) -> Result<(), SimulationExecutionErrorV1> {
        let count = u64::try_from(count).unwrap_or(u64::MAX);
        let Some(steps) = self.steps.checked_add(count) else {
            return Err(self.at(
                *site,
                SimulationExecutionErrorKindV1::StepLimit {
                    limit: self.limits.max_steps,
                },
            ));
        };
        if steps > self.limits.max_steps {
            return Err(self.at(
                *site,
                SimulationExecutionErrorKindV1::StepLimit {
                    limit: self.limits.max_steps,
                },
            ));
        }
        self.steps = steps;
        Ok(())
    }

    fn event(
        &mut self,
        site: &CompactSite,
        kind: SimulationEventKindV1,
    ) -> Result<(), SimulationExecutionErrorV1> {
        if self.policy == EventPolicyV1::Disabled || self.event_delivery_stopped {
            return Ok(());
        }
        let site = self.materialize_event_site(*site);
        let function = &self.module.functions[site.function_ordinal].id;
        deliver_event(
            self.policy,
            self.limits.max_events,
            &mut self.events,
            &mut self.reserved_event_closures,
            &mut self.event_delivery_stopped,
            self.invocation,
            self.sink,
            function,
            &site,
            kind,
            false,
        )
    }

    fn call_event(
        &mut self,
        site: &CompactSite,
        callee: usize,
    ) -> Result<(), SimulationExecutionErrorV1> {
        if self.policy == EventPolicyV1::Disabled || self.event_delivery_stopped {
            return Ok(());
        }
        self.event(
            site,
            SimulationEventKindV1::Call {
                callee_ordinal: self.function_module_indices[callee],
            },
        )
    }

    fn begin_lifecycle(
        &mut self,
        site: &CompactSite,
        kind: SimulationEventKindV1,
    ) -> Result<(), SimulationExecutionErrorV1> {
        let reserved = self.reserve_event_closure(site)?;
        self.emit_reserved_begin(site, kind, reserved)
    }

    fn reserve_event_closure(
        &mut self,
        site: &CompactSite,
    ) -> Result<bool, SimulationExecutionErrorV1> {
        if self.policy == EventPolicyV1::Disabled || self.event_delivery_stopped {
            return Ok(false);
        }
        let required = self
            .events
            .checked_add(self.reserved_event_closures)
            .and_then(|events| events.checked_add(2))
            .ok_or_else(|| {
                self.at(
                    *site,
                    SimulationExecutionErrorKindV1::EventLimit {
                        limit: self.limits.max_events,
                    },
                )
            })?;
        if required > self.limits.max_events {
            return Err(self.at(
                *site,
                SimulationExecutionErrorKindV1::EventLimit {
                    limit: self.limits.max_events,
                },
            ));
        }
        self.reserved_event_closures += 1;
        Ok(true)
    }

    fn emit_reserved_begin(
        &mut self,
        site: &CompactSite,
        kind: SimulationEventKindV1,
        reserved: bool,
    ) -> Result<(), SimulationExecutionErrorV1> {
        let result = self.event(site, kind);
        if result.is_err() && reserved {
            self.reserved_event_closures = self.reserved_event_closures.saturating_sub(1);
        }
        result
    }

    fn end_lifecycle(
        &mut self,
        site: &CompactSite,
        kind: SimulationEventKindV1,
    ) -> Result<(), SimulationExecutionErrorV1> {
        if self.policy == EventPolicyV1::Disabled || self.event_delivery_stopped {
            return Ok(());
        }
        let site = self.materialize_event_site(*site);
        let function = &self.module.functions[site.function_ordinal].id;
        deliver_event(
            self.policy,
            self.limits.max_events,
            &mut self.events,
            &mut self.reserved_event_closures,
            &mut self.event_delivery_stopped,
            self.invocation,
            self.sink,
            function,
            &site,
            kind,
            true,
        )
    }

    fn cancel_event_closure(&mut self, reserved: bool) {
        if reserved {
            self.reserved_event_closures = self.reserved_event_closures.saturating_sub(1);
        }
    }

    fn observe_and_commit_store(
        &mut self,
        site: &CompactSite,
        pointer: &PointerValue,
        value: ScalarBitsV1,
        width: usize,
    ) -> Result<(), SimulationExecutionErrorV1> {
        let observed_site = (self.policy == EventPolicyV1::Enabled && !self.event_delivery_stopped)
            .then(|| self.materialize_event_site(*site));
        let prepared = match self.memory.prepare_store(pointer, width) {
            Ok(prepared) => prepared,
            Err(kind) => {
                return Err(SimulationExecutionErrorV1 {
                    invocation: self.invocation,
                    site: Some(self.materialize_site(*site)),
                    kind,
                    observation_failure: None,
                });
            }
        };
        if let Some(observed_site) = observed_site {
            let function = &self.module.functions[observed_site.function_ordinal].id;
            deliver_event(
                self.policy,
                self.limits.max_events,
                &mut self.events,
                &mut self.reserved_event_closures,
                &mut self.event_delivery_stopped,
                self.invocation,
                self.sink,
                function,
                &observed_site,
                SimulationEventKindV1::MemoryWrite {
                    allocation: pointer.allocation,
                    offset: pointer.byte_offset,
                    bytes: width,
                },
                false,
            )?;
        }
        prepared.commit(value);
        if pointer.address_space == AddressSpace::Workgroup {
            let invocation = self.invocation.ok_or_else(|| {
                self.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant("workgroup store invocation"),
                )
            })?;
            self.memory
                .mark_workgroup_store(pointer, width, invocation)
                .map_err(|kind| self.at(*site, kind))?;
        }
        self.debug_memory(
            *site,
            SimulationDebugMemoryAccessV1::WriteCommitted,
            pointer,
            width,
            value,
        );
        Ok(())
    }

    fn workgroup_pointer(
        &mut self,
        site: CompactSite,
        memory: &fe2o3_kernel_ir::WorkgroupMemory,
    ) -> Result<PointerValue, SimulationExecutionErrorV1> {
        let Type::Scalar(element) = memory.element else {
            return Err(self.at(
                site,
                SimulationExecutionErrorKindV1::InternalInvariant(
                    "preflighted scalar workgroup memory",
                ),
            ));
        };
        let WorkgroupMemoryExtent::Static(elements) = memory.extent else {
            return Err(self.at(
                site,
                SimulationExecutionErrorKindV1::InternalInvariant(
                    "preflighted static workgroup memory",
                ),
            ));
        };
        let element_bytes = self.target.scalar_bytes(element).ok_or_else(|| {
            self.at(
                site,
                SimulationExecutionErrorKindV1::InternalInvariant(
                    "preflighted workgroup memory element",
                ),
            )
        })?;
        let bytes = usize::try_from(elements)
            .ok()
            .and_then(|elements| elements.checked_mul(element_bytes))
            .ok_or_else(|| {
                self.at(
                    site,
                    SimulationExecutionErrorKindV1::AllocationBytesLimit {
                        actual: usize::MAX,
                        limit: self.limits.max_allocation_bytes,
                    },
                )
            })?;
        if let Some(existing) = self
            .workgroup_allocations
            .iter()
            .find(|allocation| allocation.site == site)
        {
            if existing.element != element || existing.bytes != bytes {
                return Err(self.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "workgroup allocation site shape changed",
                    ),
                ));
            }
            return Ok(PointerValue {
                allocation: existing.id,
                byte_offset: 0,
                element,
                address_space: AddressSpace::Workgroup,
                access: AccessMode::ReadWrite,
                lower_bound: 0,
                upper_bound: bytes,
            });
        }

        self.memory
            .validate_allocation(bytes, self.limits)
            .map_err(|kind| self.at(site, kind))?;
        if self.workgroup_allocations.len() == self.workgroup_allocations.capacity()
            && self.workgroup_allocations.try_reserve_exact(1).is_err()
        {
            return Err(self.at(site, SimulationExecutionErrorKindV1::AllocationFailure));
        }
        let allocation_bytes = try_filled(bytes, 0_u8).map_err(|kind| self.at(site, kind))?;
        let initialized = try_filled(bytes, false).map_err(|kind| self.at(site, kind))?;
        let reserved = self.reserve_event_closure(&site)?;
        let id = match self.memory.allocate(
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
            memory.alignment,
            allocation_bytes,
            initialized,
            self.limits,
        ) {
            Ok(id) => id,
            Err(kind) => {
                self.cancel_event_closure(reserved);
                return Err(self.at(site, kind));
            }
        };
        self.workgroup_allocations.push(WorkgroupAllocation {
            site,
            id,
            element,
            bytes,
            lifecycle_observed: reserved,
        });
        self.emit_reserved_begin(
            &site,
            SimulationEventKindV1::AllocationCreated {
                allocation: id,
                address_space: AddressSpace::Workgroup,
                bytes,
            },
            reserved,
        )?;
        Ok(PointerValue {
            allocation: id,
            byte_offset: 0,
            element,
            address_space: AddressSpace::Workgroup,
            access: AccessMode::ReadWrite,
            lower_bound: 0,
            upper_bound: bytes,
        })
    }

    fn publish_workgroup(&mut self) {
        self.memory.publish_workgroup();
    }

    fn publish_global_happens_before(&mut self, barrier: &WorkgroupBarrier) {
        if barrier
            .semantics
            .address_spaces
            .contains(&AddressSpace::Global)
            && matches!(
                barrier.semantics.ordering,
                MemoryOrdering::AcquireRelease | MemoryOrdering::SequentiallyConsistent
            )
        {
            self.workgroup_happens_before_epoch =
                self.workgroup_happens_before_epoch.saturating_add(1);
        }
    }

    fn release_workgroup_allocations(
        &mut self,
        primary: &mut Option<&mut SimulationExecutionErrorV1>,
    ) -> Result<(), SimulationExecutionErrorV1> {
        while let Some(allocation) = self.workgroup_allocations.pop() {
            let released = self
                .memory
                .release_one(allocation.id)
                .map_err(|kind| self.at(allocation.site, kind))?;
            if !released {
                return Err(self.at(
                    allocation.site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "workgroup allocation remained live until release",
                    ),
                ));
            }
            if allocation.lifecycle_observed
                && let Err(secondary) = self.end_lifecycle(
                    &allocation.site,
                    SimulationEventKindV1::AllocationReleased {
                        allocation: allocation.id,
                    },
                )
            {
                if let Some(primary) = primary.as_deref_mut() {
                    attach_observation_failure(primary, secondary);
                } else {
                    return Err(secondary);
                }
            }
        }
        Ok(())
    }

    fn record_access(
        &mut self,
        site: &CompactSite,
        allocation: u64,
        offset: usize,
        bytes: usize,
        write: bool,
        atomic: bool,
    ) -> Result<(), SimulationExecutionErrorV1> {
        let invocation = self.invocation.ok_or_else(|| {
            self.at(
                *site,
                SimulationExecutionErrorKindV1::InternalInvariant("memory access invocation"),
            )
        })?;
        let end = offset
            .checked_add(bytes)
            .ok_or_else(|| self.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow))?;
        let compact_site = *site;
        for byte in offset..end {
            let key = (allocation, byte);
            let previous = self.accesses.get(&key).copied();
            let mut frontier = previous.unwrap_or_default();
            let candidates = if write {
                [
                    frontier.write,
                    frontier.displaced_write,
                    frontier.read,
                    frontier.displaced_read,
                ]
            } else {
                [frontier.write, frontier.displaced_write, None, None]
            };
            let mut conflicting = false;
            let mut racing = false;
            for earlier_access in candidates
                .into_iter()
                .flatten()
                .filter(|earlier| earlier.invocation != invocation)
            {
                conflicting = true;
                let conflict_evidence = SimulationMemoryConflictV1 {
                    allocation,
                    offset: byte,
                    earlier: earlier_access.invocation,
                    later: invocation,
                    earlier_site: self.materialize_site(earlier_access.site),
                    later_site: self.materialize_site(*site),
                };
                if self.first_conflict.is_none() {
                    self.first_conflict = Some(conflict_evidence.clone());
                }
                let ordered = if earlier_access.atomic && atomic {
                    Some(SimulationHappensBeforeReasonV1::AtomicSerialization)
                } else if earlier_access.invocation.workgroup == invocation.workgroup
                    && earlier_access.happens_before_epoch < self.workgroup_happens_before_epoch
                {
                    Some(SimulationHappensBeforeReasonV1::GlobalWorkgroupBarrier)
                } else {
                    None
                };
                if self.race_trackers.is_empty() {
                    self.race_trackers.try_reserve_exact(1).map_err(|_| {
                        self.at(*site, SimulationExecutionErrorKindV1::AllocationFailure)
                    })?;
                    self.race_trackers.push(RaceTracker {
                        racing_bytes: 0,
                        first_race: None,
                        first_ordered_conflict: None,
                    });
                }
                let missing_tracker = self.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "recorded memory conflict race tracker",
                    ),
                );
                let race = self.race_trackers.first_mut().ok_or(missing_tracker)?;
                if let Some(reason) = ordered {
                    if race.first_ordered_conflict.is_none() {
                        race.first_ordered_conflict = Some(SimulationOrderedMemoryConflictV1 {
                            conflict: conflict_evidence,
                            reason,
                        });
                    }
                } else {
                    racing = true;
                    if race.first_race.is_none() {
                        race.first_race = Some(SimulationDataRaceV1 {
                            conflict: conflict_evidence,
                            earlier_atomic: earlier_access.atomic,
                            later_atomic: atomic,
                        });
                    }
                }
            }
            if conflicting && !frontier.conflicted {
                self.conflicting_bytes = self.conflicting_bytes.saturating_add(1);
            }
            if racing && !frontier.raced {
                let missing_tracker = self.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant("recorded data race tracker"),
                );
                let race = self.race_trackers.first_mut().ok_or(missing_tracker)?;
                race.racing_bytes = race.racing_bytes.saturating_add(1);
            }
            frontier.conflicted |= conflicting;
            frontier.raced |= racing;
            if frontier.raced {
                // A proven race fixes this byte's race classification and
                // unique-byte count even if older representatives were lost.
                frontier.incomplete = false;
            } else if (frontier.lost_write && !(atomic && frontier.lost_writes_all_atomic))
                || (write && frontier.lost_read && !(atomic && frontier.lost_reads_all_atomic))
            {
                // A prior bounded-frontier eviction can matter only once a
                // later access is not known to serialize with every lost
                // representative. Atomic-only histories remain exact.
                frontier.incomplete = true;
            }

            if previous.is_some() || self.accesses.len() < self.limits.max_memory_access_records {
                let slot = if write {
                    if let Some(earlier) = frontier.write
                        && earlier.invocation != invocation
                        && !frontier.raced
                    {
                        if frontier
                            .displaced_write
                            .is_some_and(|displaced| displaced.invocation != earlier.invocation)
                        {
                            let displaced = frontier
                                .displaced_write
                                .expect("checked displaced write frontier");
                            frontier.lost_writes_all_atomic = if frontier.lost_write {
                                frontier.lost_writes_all_atomic && displaced.atomic
                            } else {
                                displaced.atomic
                            };
                            frontier.lost_write = true;
                        }
                        // Keep the immediately displaced writer so an
                        // ordinary access by the replacement writer is still
                        // compared with its atomic predecessor.
                        frontier.displaced_write = Some(earlier);
                    }
                    &mut frontier.write
                } else {
                    if let Some(earlier) = frontier.read
                        && earlier.invocation != invocation
                        && !frontier.raced
                    {
                        if frontier
                            .displaced_read
                            .is_some_and(|displaced| displaced.invocation != earlier.invocation)
                        {
                            let displaced = frontier
                                .displaced_read
                                .expect("checked displaced read frontier");
                            frontier.lost_reads_all_atomic = if frontier.lost_read {
                                frontier.lost_reads_all_atomic && displaced.atomic
                            } else {
                                displaced.atomic
                            };
                            frontier.lost_read = true;
                        }
                        frontier.displaced_read = Some(earlier);
                    }
                    &mut frontier.read
                };
                *slot = Some(match *slot {
                    Some(earlier)
                        if earlier.invocation == invocation
                            && earlier.happens_before_epoch
                                == self.workgroup_happens_before_epoch =>
                    {
                        LastAccess {
                            invocation,
                            site: earlier.site,
                            atomic: earlier.atomic && atomic,
                            happens_before_epoch: earlier.happens_before_epoch,
                        }
                    }
                    _ => LastAccess {
                        invocation,
                        site: compact_site,
                        atomic,
                        happens_before_epoch: self.workgroup_happens_before_epoch,
                    },
                });
                self.accesses.insert(key, frontier);
            } else {
                self.conflict_incomplete = true;
            }
        }
        Ok(())
    }

    fn conflict_assessment(&self) -> SimulationConflictAssessmentV1 {
        let access_frontier_incomplete = self.accesses.values().any(|frontier| frontier.incomplete);
        if self.conflict_incomplete || access_frontier_incomplete {
            SimulationConflictAssessmentV1::Incomplete {
                conflicting_bytes: self.conflicting_bytes,
                first: self.first_conflict.clone(),
                access_record_limit_reached: self.conflict_incomplete,
                access_frontier_incomplete,
                record_limit: self.limits.max_memory_access_records,
            }
        } else if let Some(first) = self.first_conflict.clone() {
            SimulationConflictAssessmentV1::ConflictsObserved {
                conflicting_bytes: self.conflicting_bytes,
                first,
            }
        } else {
            SimulationConflictAssessmentV1::NoConflictsObserved
        }
    }

    fn race_assessment(&self) -> SimulationRaceAssessmentV1 {
        let race = self.race_trackers.first();
        let access_frontier_incomplete = self.accesses.values().any(|frontier| frontier.incomplete);
        let synchronization_incomplete = self.unmodeled_atomic_or_fence_happens_before
            && race.is_some_and(|race| race.first_race.is_some());
        if self.conflict_incomplete || access_frontier_incomplete || synchronization_incomplete {
            return SimulationRaceAssessmentV1::Incomplete {
                racing_bytes: race.map_or(0, |race| race.racing_bytes),
                first: race.and_then(|race| race.first_race.clone()),
                first_ordered_conflict: race.and_then(|race| race.first_ordered_conflict.clone()),
                access_record_limit_reached: self.conflict_incomplete,
                access_frontier_incomplete,
                atomic_or_fence_happens_before_unmodeled: synchronization_incomplete,
                record_limit: self.limits.max_memory_access_records,
            };
        }
        let Some(race) = race else {
            return SimulationRaceAssessmentV1::NoRacesObserved {
                first_ordered_conflict: None,
            };
        };
        if let Some(first) = race.first_race.clone() {
            SimulationRaceAssessmentV1::RacesObserved {
                racing_bytes: race.racing_bytes,
                first,
                first_ordered_conflict: race.first_ordered_conflict.clone(),
            }
        } else {
            SimulationRaceAssessmentV1::NoRacesObserved {
                first_ordered_conflict: race.first_ordered_conflict.clone(),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn deliver_event<S: SimulationEventSinkV1>(
    policy: EventPolicyV1,
    max_events: u64,
    events: &mut u64,
    reserved_closures: &mut u64,
    delivery_stopped: &mut bool,
    invocation: Option<SimulationInvocationV1>,
    sink: &mut S,
    function: &FunctionId,
    site: &SimulationEventSiteV1,
    kind: SimulationEventKindV1,
    closure: bool,
) -> Result<(), SimulationExecutionErrorV1> {
    if policy == EventPolicyV1::Disabled || *delivery_stopped {
        return Ok(());
    }
    if closure {
        if *reserved_closures == 0 {
            return Err(SimulationExecutionErrorV1 {
                invocation,
                site: Some(owned_event_site(function, site)),
                kind: SimulationExecutionErrorKindV1::InternalInvariant(
                    "lifecycle closure event credit",
                ),
                observation_failure: None,
            });
        }
        *reserved_closures -= 1;
    } else if events
        .checked_add(*reserved_closures)
        .is_none_or(|required| required >= max_events)
    {
        return Err(SimulationExecutionErrorV1 {
            invocation,
            site: Some(owned_event_site(function, site)),
            kind: SimulationExecutionErrorKindV1::EventLimit { limit: max_events },
            observation_failure: None,
        });
    }
    let invocation = invocation.ok_or_else(|| SimulationExecutionErrorV1 {
        invocation: None,
        site: Some(owned_event_site(function, site)),
        kind: SimulationExecutionErrorKindV1::InternalInvariant("event invocation"),
        observation_failure: None,
    })?;
    let control = match sink.record_controlled(&SimulationEventV1 {
        invocation,
        site: *site,
        kind,
    }) {
        Ok(control) => control,
        Err(error) => {
            *delivery_stopped = true;
            *reserved_closures = 0;
            return Err(SimulationExecutionErrorV1 {
                invocation: Some(invocation),
                site: Some(owned_event_site(function, site)),
                kind: SimulationExecutionErrorKindV1::EventSinkFailure(error),
                observation_failure: None,
            });
        }
    };
    match control {
        SimulationEventSinkControlV1::Continue => *events += 1,
        SimulationEventSinkControlV1::Stop => {
            *events += 1;
            *delivery_stopped = true;
            *reserved_closures = 0;
        }
        SimulationEventSinkControlV1::DropAndStop => {
            *delivery_stopped = true;
            *reserved_closures = 0;
        }
    }
    Ok(())
}

fn owned_event_site(function: &FunctionId, site: &SimulationEventSiteV1) -> SimulationSiteV1 {
    SimulationSiteV1 {
        function: function.clone(),
        block: site.block,
        operation: site.operation,
    }
}

type ExecutionIndices<'a> = (
    HashMap<&'a FunctionId, usize>,
    Vec<usize>,
    Vec<HashMap<BlockId, usize>>,
    Vec<usize>,
    Vec<Vec<Vec<CallTarget>>>,
    Vec<Vec<SwitchLookup>>,
);

fn execution_indices_resident_bytes(indices: &ExecutionIndices<'_>) -> Option<usize> {
    let (functions, module_indices, blocks, ssa_values, call_targets, switch_targets) = indices;
    let mut resident = ResidentLedger::new(0);
    resident.add_bytes(hash_map_capacity_bytes::<&FunctionId, usize>(
        functions.capacity(),
    )?)?;
    resident.add_vec::<usize>(module_indices.capacity())?;
    resident.add_vec::<HashMap<BlockId, usize>>(blocks.capacity())?;
    for function_blocks in blocks {
        resident.add_bytes(hash_map_capacity_bytes::<BlockId, usize>(
            function_blocks.capacity(),
        )?)?;
    }
    resident.add_vec::<usize>(ssa_values.capacity())?;
    resident.add_vec::<Vec<Vec<CallTarget>>>(call_targets.capacity())?;
    for function_calls in call_targets {
        resident.add_vec::<Vec<CallTarget>>(function_calls.capacity())?;
        for block_calls in function_calls {
            resident.add_vec::<CallTarget>(block_calls.capacity())?;
        }
    }
    resident.add_vec::<Vec<SwitchLookup>>(switch_targets.capacity())?;
    for function_switches in switch_targets {
        resident.add_vec::<SwitchLookup>(function_switches.capacity())?;
        for lookup in function_switches {
            match lookup {
                SwitchLookup::None => {}
                SwitchLookup::Index(map) => {
                    resident.add_bytes(hash_map_capacity_bytes::<u128, usize>(map.capacity())?)?
                }
                SwitchLookup::Integer(map) => {
                    resident.add_bytes(hash_map_capacity_bytes::<(ScalarType, u128), usize>(
                        map.capacity(),
                    )?)?
                }
            }
        }
    }
    Some(resident.bytes())
}

pub(crate) fn preflight_execution_indices_resident_bytes(
    module: &Module,
    reachable: &[usize],
    target: SimulationTargetV1,
) -> Result<Option<usize>, SimulationPreflightErrorV1> {
    let indices = build_execution_indices(module, reachable, target)
        .map_err(|_| SimulationPreflightErrorV1::AllocationFailure)?;
    Ok(execution_indices_resident_bytes(&indices))
}

fn build_execution_indices<'a>(
    module: &'a Module,
    reachable: &[usize],
    target: SimulationTargetV1,
) -> Result<ExecutionIndices<'a>, SimulationExecutionErrorV1> {
    let mut functions = HashMap::new();
    functions
        .try_reserve(reachable.len())
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    let mut module_indices = Vec::new();
    module_indices
        .try_reserve_exact(reachable.len())
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(reachable.len())
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    let mut ssa_values = Vec::new();
    ssa_values
        .try_reserve_exact(reachable.len())
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    for (function_index, module_index) in reachable.iter().copied().enumerate() {
        let function = &module.functions[module_index];
        functions.insert(&function.id, function_index);
        module_indices.push(module_index);
    }
    let mut call_targets = Vec::new();
    call_targets
        .try_reserve_exact(reachable.len())
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    let mut switch_targets = Vec::new();
    switch_targets
        .try_reserve_exact(reachable.len())
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    for module_index in reachable.iter().copied() {
        let function = &module.functions[module_index];
        let block_count = function.body.as_ref().map_or(0, |body| body.blocks.len());
        let mut function_blocks = HashMap::new();
        function_blocks
            .try_reserve(block_count)
            .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
        if let Some(body) = &function.body {
            for (block_index, block) in body.blocks.iter().enumerate() {
                function_blocks.insert(block.id, block_index);
            }
        }
        blocks.push(function_blocks);
        ssa_values.push(function_ssa_definition_count(function).unwrap_or(0));
        let mut function_calls = Vec::new();
        function_calls
            .try_reserve_exact(block_count)
            .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                let mut block_calls = Vec::new();
                block_calls
                    .try_reserve_exact(block.operations.len())
                    .map_err(|_| {
                        top_level_error(SimulationExecutionErrorKindV1::AllocationFailure)
                    })?;
                for operation in &block.operations {
                    let target = match &operation.kind {
                        OperationKind::Call { callee, arguments } => {
                            if let Some(operation) =
                                crate::soft_float::operation_for_call_v1(callee, arguments)
                            {
                                CallTarget::Float(operation)
                            } else {
                                CallTarget::Internal(*functions.get(callee).ok_or_else(|| {
                                    top_level_error(
                                        SimulationExecutionErrorKindV1::MissingFunction(
                                            callee.clone(),
                                        ),
                                    )
                                })?)
                            }
                        }
                        _ => CallTarget::NotCall,
                    };
                    block_calls.push(target);
                }
                function_calls.push(block_calls);
            }
        }
        call_targets.push(function_calls);
        let mut function_switches = Vec::new();
        function_switches
            .try_reserve_exact(block_count)
            .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                let lookup = match block.terminator.as_ref() {
                    Some(Terminator::Switch { cases, .. }) => {
                        let mut lookup = HashMap::new();
                        lookup.try_reserve(cases.len()).map_err(|_| {
                            top_level_error(SimulationExecutionErrorKindV1::AllocationFailure)
                        })?;
                        for (case_index, case) in cases.iter().enumerate() {
                            lookup.insert(u128::from(case.value), case_index);
                        }
                        SwitchLookup::Index(lookup)
                    }
                    Some(Terminator::IntegerSwitch { cases, .. }) => {
                        let mut lookup = HashMap::new();
                        lookup.try_reserve(cases.len()).map_err(|_| {
                            top_level_error(SimulationExecutionErrorKindV1::AllocationFailure)
                        })?;
                        for (case_index, case) in cases.iter().enumerate() {
                            let value =
                                constant_scalar(&case.value, target).map_err(top_level_error)?;
                            lookup.insert((value.ty(), value.bits()), case_index);
                        }
                        SwitchLookup::Integer(lookup)
                    }
                    _ => SwitchLookup::None,
                };
                function_switches.push(lookup);
            }
        }
        switch_targets.push(function_switches);
    }
    Ok((
        functions,
        module_indices,
        blocks,
        ssa_values,
        call_targets,
        switch_targets,
    ))
}

struct ExecutionConfiguration<'a> {
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    policy: EventPolicyV1,
    plan: SimulationPlanV1,
    debug_capture: SimulationDebugCaptureLimitsV1,
    schedule: Option<SimulationScheduleRequestV1<'a>>,
    resident_offset: usize,
}

fn execute(
    admitted: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    configuration: ExecutionConfiguration<'_>,
    sink: &mut impl SimulationEventSinkV1,
    debug_sink: &mut impl SimulationDebugSinkV1,
) -> Result<SimulationExecutionV1, SimulationExecutionErrorV1> {
    let ExecutionConfiguration {
        target,
        limits,
        policy,
        plan,
        debug_capture,
        schedule,
        resident_offset,
    } = configuration;
    let workgroup_participants = usize::try_from(plan.workgroup[0])
        .ok()
        .and_then(|x| x.checked_mul(plan.workgroup[1] as usize))
        .and_then(|xy| xy.checked_mul(plan.workgroup[2] as usize))
        .ok_or_else(|| {
            top_level_error(SimulationExecutionErrorKindV1::InternalInvariant(
                "preflighted workgroup participant count",
            ))
        })?;
    let mut schedule = PreparedScheduleV1::prepare(
        schedule,
        admitted.identity,
        request,
        target,
        limits,
        &plan,
        workgroup_participants,
        resident_offset,
    )
    .map_err(|error| top_level_error(schedule_prepare_error(error)))?;
    let indices =
        build_execution_indices(&admitted.module, &plan.reachable_function_indices, target)?;
    let actual_index_resident_bytes =
        execution_indices_resident_bytes(&indices).ok_or_else(|| {
            top_level_error(SimulationExecutionErrorKindV1::InternalInvariant(
                "execution index resident accounting overflow",
            ))
        })?;
    if actual_index_resident_bytes > plan.execution_index_resident_bytes {
        return Err(top_level_error(
            SimulationExecutionErrorKindV1::InternalInvariant(
                "execution index resident capacity exceeded preflight plan",
            ),
        ));
    }
    let (
        function_indices,
        function_module_indices,
        block_indices,
        function_ssa_values,
        call_targets,
        switch_targets,
    ) = indices;
    let entry_index = function_indices
        .get(&plan.kernel.entry)
        .copied()
        .ok_or_else(|| SimulationExecutionErrorV1 {
            invocation: None,
            site: None,
            kind: SimulationExecutionErrorKindV1::MissingFunction(plan.kernel.entry.clone()),
            observation_failure: None,
        })?;
    let entry_module_index = function_module_indices[entry_index];
    drop(function_indices);
    let entry = &admitted.module.functions[entry_module_index];
    let memory = Memory::new(
        request.arguments.len(),
        request.shared_buffers.len(),
        limits,
    )
    .map_err(top_level_error)?;
    let mut accesses = HashMap::new();
    accesses
        .try_reserve(limits.max_memory_access_records)
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    let mut workgroup_allocations = Vec::new();
    workgroup_allocations
        .try_reserve_exact(plan.workgroup_allocation_sites)
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    let mut engine = Engine {
        module: &admitted.module,
        function_module_indices,
        block_indices,
        function_ssa_values,
        call_targets,
        switch_targets,
        target,
        limits,
        policy,
        memory,
        sink,
        debug_capture,
        debug_sink,
        debug_records: 0,
        debug_delivery_stopped: !debug_capture.is_enabled(),
        schedule_identity: schedule.identity(),
        schedule_decision: 0,
        steps: 0,
        events: 0,
        reserved_event_closures: 0,
        event_delivery_stopped: false,
        invocation: None,
        accesses,
        conflicting_bytes: 0,
        first_conflict: None,
        conflict_incomplete: false,
        workgroup_happens_before_epoch: 0,
        unmodeled_atomic_or_fence_happens_before: false,
        race_trackers: Vec::new(),
        workgroup_allocations,
    };
    initialize_shared_buffers(&mut engine, request)?;
    let parameters = initialize_arguments(&mut engine, entry, request)?;
    let invocation_site = entry
        .body
        .as_ref()
        .and_then(|body| body.blocks.first())
        .map(|block| terminator_site(entry_index, block))
        .ok_or_else(|| {
            engine.fail(SimulationExecutionErrorKindV1::MissingBody(
                entry.id.clone(),
            ))
        })?;
    let mut invocations = 0_u64;
    let mut workgroups = 0_u64;
    let mut scheduled_slots = 0_u64;
    let mut machines = Vec::new();
    machines
        .try_reserve_exact(workgroup_participants)
        .map_err(|_| engine.fail(SimulationExecutionErrorKindV1::AllocationFailure))?;

    for group_z in 0..plan.workgroup_count[2] {
        for group_y in 0..plan.workgroup_count[1] {
            for group_x in 0..plan.workgroup_count[0] {
                workgroups += 1;
                schedule.begin_workgroup();
                engine.workgroup_happens_before_epoch = 0;
                machines.clear();
                for local_z in 0..plan.workgroup[2] {
                    for local_y in 0..plan.workgroup[1] {
                        for local_x in 0..plan.workgroup[0] {
                            scheduled_slots += 1;
                            let global = [
                                group_x * u64::from(plan.workgroup[0]) + u64::from(local_x),
                                group_y * u64::from(plan.workgroup[1]) + u64::from(local_y),
                                group_z * u64::from(plan.workgroup[2]) + u64::from(local_z),
                            ];
                            if global
                                .iter()
                                .zip(plan.grid)
                                .any(|(coordinate, extent)| *coordinate >= extent)
                            {
                                continue;
                            }
                            let invocation = SimulationInvocationV1 {
                                global,
                                workgroup: [group_x, group_y, group_z],
                                local: [local_x, local_y, local_z],
                                workgroup_size: plan.workgroup,
                                workgroup_count: plan.workgroup_count,
                                launch_extent: plan.grid,
                            };
                            engine.invocation = Some(invocation);
                            machines.push(InvocationMachine::new(
                                &engine,
                                invocation,
                                entry_index,
                                entry,
                                &parameters,
                            )?);
                        }
                    }
                }

                let begun = begin_workgroup_invocations(
                    &mut engine,
                    &mut machines,
                    &invocation_site,
                    invocations == 0,
                )?;
                debug_assert_eq!(begun, machines.len());
                let execution = if schedule.uses_canonical_order() {
                    execute_cooperative_workgroup(
                        &mut engine,
                        &mut machines,
                        &invocation_site,
                        &mut invocations,
                        &mut schedule,
                        true,
                    )
                } else {
                    execute_cooperative_workgroup(
                        &mut engine,
                        &mut machines,
                        &invocation_site,
                        &mut invocations,
                        &mut schedule,
                        false,
                    )
                };
                if let Err(mut error) = execution {
                    abort_workgroup(
                        &mut engine,
                        &mut machines,
                        begun,
                        &invocation_site,
                        &mut error,
                    );
                    let mut primary = Some(&mut error);
                    if let Err(secondary) = engine.release_workgroup_allocations(&mut primary) {
                        attach_observation_failure(&mut error, secondary);
                    }
                    return Err(error);
                }
                let mut no_primary = None;
                engine.release_workgroup_allocations(&mut no_primary)?;
                if engine.reserved_event_closures != 0 {
                    return Err(
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "workgroup lifecycle credits were not closed",
                        )),
                    );
                }
            }
        }
    }
    if invocations != plan.invocations
        || workgroups != plan.workgroups
        || scheduled_slots != plan.scheduled_slots
    {
        return Err(
            engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                "canonical scheduler count mismatch",
            )),
        );
    }
    let arguments = copy_back_arguments(&engine.memory, &request.arguments)?;
    let shared_buffers = copy_back_shared_buffers(&engine.memory, &request.shared_buffers)?;
    let conflict_assessment = engine.conflict_assessment();
    let schedule = schedule
        .finish(plan.workgroups, admitted.identity, request, target, limits)
        .map_err(|error| engine.fail(SimulationExecutionErrorKindV1::ScheduleReplay(error)))?;
    let supplemental = collect_supplemental_observations(&engine, schedule.records)?;
    Ok(SimulationExecutionV1 {
        identity: admitted.identity,
        arguments,
        shared_buffers,
        invocations_executed: invocations,
        workgroups_visited: workgroups,
        scheduled_slots_visited: scheduled_slots,
        steps_executed: engine.steps,
        events_emitted: engine.events,
        schedule: schedule.identity,
        schedule_transcript_identity: schedule.transcript_identity,
        schedule_coverage: schedule.coverage,
        supplemental,
        conflict_assessment,
    })
}

#[inline(never)]
fn collect_supplemental_observations(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    schedule_records: Vec<SimulationScheduleRecordV1>,
) -> Result<Vec<SimulationSupplementalV1>, SimulationExecutionErrorV1> {
    let mut supplemental = Vec::new();
    supplemental
        .try_reserve_exact(1)
        .map_err(|_| engine.fail(SimulationExecutionErrorKindV1::AllocationFailure))?;
    supplemental.push(SimulationSupplementalV1 {
        schedule_records,
        race_assessment: engine.race_assessment(),
    });
    Ok(supplemental)
}

fn schedule_prepare_error(error: SchedulePrepareErrorV1) -> SimulationExecutionErrorKindV1 {
    match error {
        SchedulePrepareErrorV1::DecisionLimit { actual, limit } => {
            SimulationExecutionErrorKindV1::ScheduleDecisionLimit { actual, limit }
        }
        SchedulePrepareErrorV1::ResidentLimit { actual, limit } => {
            SimulationExecutionErrorKindV1::ScheduleResidentLimit { actual, limit }
        }
        SchedulePrepareErrorV1::AllocationFailure => {
            SimulationExecutionErrorKindV1::AllocationFailure
        }
        SchedulePrepareErrorV1::Replay(error) => {
            SimulationExecutionErrorKindV1::ScheduleReplay(error)
        }
    }
}

fn begin_workgroup_invocations<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    machines: &mut [InvocationMachine<'a>],
    invocation_site: &CompactSite,
    observe_preexisting: bool,
) -> Result<usize, SimulationExecutionErrorV1> {
    let mut begun = 0;
    for machine in machines.iter() {
        engine.invocation = Some(machine.invocation);
        if let Err(mut error) =
            engine.begin_lifecycle(invocation_site, SimulationEventKindV1::InvocationBegin)
        {
            abort_workgroup(engine, machines, begun, invocation_site, &mut error);
            return Err(error);
        }
        begun += 1;
    }
    if observe_preexisting {
        let invocation = machines.first().ok_or_else(|| {
            engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                "workgroup contained no live invocations",
            ))
        })?;
        engine.invocation = Some(invocation.invocation);
        if let Err(mut error) = observe_preexisting_allocations(engine, invocation_site) {
            abort_workgroup(engine, machines, begun, invocation_site, &mut error);
            return Err(error);
        }
    }
    Ok(begun)
}

#[inline(never)]
fn execute_cooperative_workgroup<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    machines: &mut [InvocationMachine<'a>],
    invocation_site: &CompactSite,
    invocations: &mut u64,
    schedule: &mut PreparedScheduleV1<'_>,
    canonical_order: bool,
) -> Result<(), SimulationExecutionErrorV1> {
    let workgroup = machines
        .first()
        .ok_or_else(|| {
            engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                "workgroup contained no live invocations",
            ))
        })?
        .invocation
        .workgroup;
    let mut wave_results = Vec::new();
    wave_results
        .try_reserve_exact(machines.len())
        .map_err(|_| engine.fail(SimulationExecutionErrorKindV1::AllocationFailure))?;
    let mut phase = 0_u64;
    loop {
        if canonical_order {
            for machine in machines.iter_mut() {
                if machine.completed || machine.waiting.is_some() {
                    continue;
                }
                schedule
                    .selected(machine.invocation, phase)
                    .map_err(|error| engine.fail(schedule_prepare_error(error)))?;
                engine.schedule_decision = schedule.current_decision() - 1;
                engine.invocation = Some(machine.invocation);
                match machine.advance_until_yield(engine, phase)? {
                    MachineYield::Complete => {
                        engine.end_lifecycle(
                            invocation_site,
                            SimulationEventKindV1::InvocationEnd {
                                outcome: SimulationExecutionOutcomeV1::Completed,
                            },
                        )?;
                        *invocations = invocations.checked_add(1).ok_or_else(|| {
                            engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                                "completed invocation count overflow",
                            ))
                        })?;
                        machine.completed = true;
                    }
                    MachineYield::Barrier(arrival) => {
                        machine.waiting = Some(MachineWait::Barrier(arrival));
                    }
                    MachineYield::Wave(arrival) => {
                        machine.waiting = Some(MachineWait::Wave(arrival));
                    }
                }
            }
        } else {
            let order = schedule
                .take_order(
                    machines.len(),
                    |index| machines[index].invocation,
                    |index| !machines[index].completed && machines[index].waiting.is_none(),
                    workgroup,
                    phase,
                )
                .map_err(|error| {
                    engine.invocation = None;
                    engine.fail(SimulationExecutionErrorKindV1::ScheduleReplay(error))
                })?;
            for index in order.iter().copied() {
                advance_runnable_machine(
                    engine,
                    &mut machines[index],
                    invocation_site,
                    invocations,
                    schedule,
                    phase,
                )?;
            }
            schedule.restore_order(order);
        }

        if resolve_ready_waves(engine, machines, &mut wave_results)? != 0 {
            continue;
        }
        if machines.iter().all(|machine| machine.completed) {
            return Ok(());
        }
        release_workgroup_barrier(engine, machines, schedule, &mut phase)?;
    }
}

fn advance_runnable_machine<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    machine: &mut InvocationMachine<'a>,
    invocation_site: &CompactSite,
    invocations: &mut u64,
    schedule: &mut PreparedScheduleV1<'_>,
    phase: u64,
) -> Result<(), SimulationExecutionErrorV1> {
    schedule
        .selected(machine.invocation, phase)
        .map_err(|error| engine.fail(schedule_prepare_error(error)))?;
    engine.schedule_decision = schedule.current_decision() - 1;
    engine.invocation = Some(machine.invocation);
    match machine.advance_until_yield(engine, phase)? {
        MachineYield::Complete => {
            engine.end_lifecycle(
                invocation_site,
                SimulationEventKindV1::InvocationEnd {
                    outcome: SimulationExecutionOutcomeV1::Completed,
                },
            )?;
            *invocations = invocations.checked_add(1).ok_or_else(|| {
                engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                    "completed invocation count overflow",
                ))
            })?;
            machine.completed = true;
        }
        MachineYield::Barrier(arrival) => {
            machine.waiting = Some(MachineWait::Barrier(arrival));
        }
        MachineYield::Wave(arrival) => machine.waiting = Some(MachineWait::Wave(arrival)),
    }
    Ok(())
}

fn local_linear(invocation: SimulationInvocationV1) -> u64 {
    u64::from(invocation.local[0])
        + u64::from(invocation.workgroup_size[0])
            * (u64::from(invocation.local[1])
                + u64::from(invocation.workgroup_size[1]) * u64::from(invocation.local[2]))
}

fn full_wave_mask(width: WaveWidth) -> u64 {
    match width {
        WaveWidth::Wave32 => u64::from(u32::MAX),
        WaveWidth::Wave64 => u64::MAX,
    }
}

fn wave_active_mask(machines: &[InvocationMachine<'_>], start: u64, width: u64) -> u64 {
    machines.iter().fold(0_u64, |mask, machine| {
        let linear = local_linear(machine.invocation);
        if linear >= start && linear < start + width {
            mask | (1_u64 << (linear - start))
        } else {
            mask
        }
    })
}

fn wave_member_index(machines: &[InvocationMachine<'_>], start: u64, lane: u64) -> Option<usize> {
    machines
        .iter()
        .position(|machine| local_linear(machine.invocation) == start + lane)
}

fn resolve_ready_waves<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    machines: &mut [InvocationMachine<'a>],
    results: &mut Vec<(usize, ScalarBitsV1)>,
) -> Result<usize, SimulationExecutionErrorV1> {
    let mut resolved = 0_usize;
    for representative in 0..machines.len() {
        let Some(MachineWait::Wave(arrival)) = machines[representative].waiting else {
            continue;
        };
        let width = u64::from(arrival.wave.width.lanes());
        let linear = local_linear(machines[representative].invocation);
        let wave_in_workgroup = linear / width;
        let start = wave_in_workgroup * width;
        let active_mask = wave_active_mask(machines, start, width);
        let required_mask = full_wave_mask(arrival.wave.width);
        if active_mask != required_mask {
            engine.invocation = Some(machines[representative].invocation);
            return Err(engine.at(
                arrival.site,
                SimulationExecutionErrorKindV1::IncompleteWave(IncompleteWaveV1 {
                    width: arrival.wave.width,
                    wave_in_workgroup,
                    active_mask,
                    required_mask,
                }),
            ));
        }

        for lane in 0..width {
            let index = wave_member_index(machines, start, lane).ok_or_else(|| {
                engine.at(
                    arrival.site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "full wave active mask member",
                    ),
                )
            })?;
            match machines[index].waiting {
                Some(MachineWait::Wave(peer))
                    if peer.site == arrival.site && peer.wave == arrival.wave => {}
                Some(MachineWait::Wave(peer)) => {
                    engine.invocation = Some(machines[index].invocation);
                    return Err(engine.at(
                        peer.site,
                        SimulationExecutionErrorKindV1::MismatchedWave(MismatchedWaveV1 {
                            width: arrival.wave.width,
                            expected: engine.materialize_event_site(arrival.site),
                        }),
                    ));
                }
                _ => {
                    engine.invocation = Some(machines[representative].invocation);
                    return Err(engine.at(
                        arrival.site,
                        SimulationExecutionErrorKindV1::DivergentWave(DivergentWaveV1 {
                            width: arrival.wave.width,
                            wave_in_workgroup,
                            nonparticipating: machines[index].invocation.into(),
                        }),
                    ));
                }
            }
        }

        results.clear();
        for lane in 0..width {
            let index = wave_member_index(machines, start, lane).ok_or_else(|| {
                engine.at(
                    arrival.site,
                    SimulationExecutionErrorKindV1::InternalInvariant("wave result member"),
                )
            })?;
            let Some(MachineWait::Wave(peer)) = machines[index].waiting else {
                return Err(engine.at(
                    arrival.site,
                    SimulationExecutionErrorKindV1::InternalInvariant("validated wave wait"),
                ));
            };
            let value = match peer.wave.kind {
                WaveOperationKind::LaneId => ScalarBitsV1::u32(lane as u32),
                WaveOperationKind::Ballot { .. } => {
                    let mut ballot = 0_u64;
                    for source_lane in 0..width {
                        let source =
                            wave_member_index(machines, start, source_lane).ok_or_else(|| {
                                engine.at(
                                    arrival.site,
                                    SimulationExecutionErrorKindV1::InternalInvariant(
                                        "ballot member",
                                    ),
                                )
                            })?;
                        let Some(WaveInput::Predicate(predicate)) = machines[source].wave_input()
                        else {
                            return Err(engine.at(
                                arrival.site,
                                SimulationExecutionErrorKindV1::InternalInvariant(
                                    "ballot predicate",
                                ),
                            ));
                        };
                        if predicate {
                            ballot |= 1_u64 << source_lane;
                        }
                    }
                    match peer.wave.width {
                        WaveWidth::Wave32 => ScalarBitsV1::u32(ballot as u32),
                        WaveWidth::Wave64 => {
                            ScalarBitsV1::new(ScalarType::U64, u128::from(ballot), engine.target)
                                .map_err(|_| {
                                    engine.at(
                                        arrival.site,
                                        SimulationExecutionErrorKindV1::InternalInvariant(
                                            "wave64 ballot scalar",
                                        ),
                                    )
                                })?
                        }
                    }
                }
                WaveOperationKind::Any { .. } | WaveOperationKind::All { .. } => {
                    let all = matches!(peer.wave.kind, WaveOperationKind::All { .. });
                    let mut aggregate = all;
                    for source_lane in 0..width {
                        let source =
                            wave_member_index(machines, start, source_lane).ok_or_else(|| {
                                engine.at(
                                    arrival.site,
                                    SimulationExecutionErrorKindV1::InternalInvariant(
                                        "vote member",
                                    ),
                                )
                            })?;
                        let Some(WaveInput::Predicate(predicate)) = machines[source].wave_input()
                        else {
                            return Err(engine.at(
                                arrival.site,
                                SimulationExecutionErrorKindV1::InternalInvariant("vote predicate"),
                            ));
                        };
                        if all {
                            aggregate &= predicate;
                        } else {
                            aggregate |= predicate;
                        }
                    }
                    ScalarBitsV1::boolean(aggregate)
                }
                WaveOperationKind::ShuffleIndex { .. } => {
                    let Some(WaveInput::Shuffle {
                        source_lane,
                        tile_width,
                        ..
                    }) = machines[index].wave_input()
                    else {
                        return Err(engine.at(
                            arrival.site,
                            SimulationExecutionErrorKindV1::InternalInvariant("shuffle input"),
                        ));
                    };
                    let tile_start = (lane / u64::from(tile_width)) * u64::from(tile_width);
                    let source =
                        wave_member_index(machines, start, tile_start + u64::from(source_lane))
                            .ok_or_else(|| {
                                engine.at(
                                    arrival.site,
                                    SimulationExecutionErrorKindV1::InternalInvariant(
                                        "shuffle source member",
                                    ),
                                )
                            })?;
                    let Some(WaveInput::Shuffle { value, .. }) = machines[source].wave_input()
                    else {
                        return Err(engine.at(
                            arrival.site,
                            SimulationExecutionErrorKindV1::InternalInvariant(
                                "shuffle source input",
                            ),
                        ));
                    };
                    value
                }
                WaveOperationKind::ReduceF32 { .. } | WaveOperationKind::BroadcastF32 { .. } => {
                    return Err(engine.at(
                        arrival.site,
                        SimulationExecutionErrorKindV1::InternalInvariant(
                            "unsupported wave operation passed preflight",
                        ),
                    ));
                }
            };
            results.push((index, value));
        }
        for (index, value) in results.iter().copied() {
            engine.invocation = Some(machines[index].invocation);
            machines[index].complete_wave(engine, value)?;
        }
        resolved += 1;
    }
    Ok(resolved)
}

fn release_workgroup_barrier<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    machines: &mut [InvocationMachine<'a>],
    schedule: &mut PreparedScheduleV1<'_>,
    phase: &mut u64,
) -> Result<(), SimulationExecutionErrorV1> {
    let first_waiting = machines.iter().find_map(|machine| match machine.waiting {
        Some(MachineWait::Barrier(arrival)) => Some((machine.invocation, arrival)),
        _ => None,
    });
    let Some((representative, expected)) = first_waiting else {
        return Err(engine
            .fail(SimulationExecutionErrorKindV1::WorkgroupSchedulerNoProgress { phase: *phase }));
    };
    for machine in machines.iter() {
        match machine.waiting {
            Some(MachineWait::Barrier(arrival)) => {
                if arrival.site != expected.site || arrival.barrier != expected.barrier {
                    let mismatch = match (
                        arrival.site != expected.site,
                        arrival.barrier != expected.barrier,
                    ) {
                        (true, true) => WorkgroupBarrierMismatchV1::SiteAndSemantics,
                        (true, false) => WorkgroupBarrierMismatchV1::Site,
                        (false, true) => WorkgroupBarrierMismatchV1::Semantics,
                        (false, false) => unreachable!(),
                    };
                    engine.invocation = Some(machine.invocation);
                    return Err(engine.at(
                        arrival.site,
                        SimulationExecutionErrorKindV1::MismatchedWorkgroupBarrier(
                            MismatchedWorkgroupBarrierV1 {
                                phase: *phase,
                                expected: engine.materialize_event_site(expected.site),
                                mismatch,
                            },
                        ),
                    ));
                }
            }
            _ => {
                engine.invocation = Some(representative);
                return Err(engine.at(
                    expected.site,
                    SimulationExecutionErrorKindV1::DivergentWorkgroupBarrier(
                        DivergentWorkgroupBarrierV1 {
                            phase: *phase,
                            waiting: representative.into(),
                            exited: machine.invocation.into(),
                        },
                    ),
                ));
            }
        }
    }
    let participants = u32::try_from(machines.len()).map_err(|_| {
        engine.at(
            expected.site,
            SimulationExecutionErrorKindV1::InternalInvariant("workgroup participant count"),
        )
    })?;
    engine.invocation = Some(representative);
    engine.event(
        &expected.site,
        SimulationEventKindV1::WorkgroupBarrierRelease {
            phase: *phase,
            participants,
        },
    )?;
    engine.debug_barrier(
        expected.site,
        SimulationDebugBarrierActionV1::Release,
        *phase,
        participants,
    );
    schedule.barrier_released();
    engine.publish_workgroup();
    engine.publish_global_happens_before(expected.barrier);
    for machine in machines.iter_mut() {
        machine.waiting = None;
    }
    *phase = phase.checked_add(1).ok_or_else(|| {
        engine.at(
            expected.site,
            SimulationExecutionErrorKindV1::StepLimit {
                limit: engine.limits.max_steps,
            },
        )
    })?;
    Ok(())
}

fn abort_workgroup<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    machines: &mut [InvocationMachine<'a>],
    begun: usize,
    invocation_site: &CompactSite,
    primary: &mut SimulationExecutionErrorV1,
) {
    for machine in machines.iter_mut().take(begun) {
        if machine.completed {
            continue;
        }
        engine.invocation = Some(machine.invocation);
        if machine.active_depth != 0 {
            machine.frames.truncate(machine.active_depth);
            unwind_frames(engine, &mut machine.frames, primary);
            machine.active_depth = 0;
        }
        if let Err(secondary) = engine.end_lifecycle(
            invocation_site,
            SimulationEventKindV1::InvocationEnd {
                outcome: SimulationExecutionOutcomeV1::Failed,
            },
        ) {
            attach_observation_failure(primary, secondary);
        }
    }
}

fn observe_preexisting_allocations(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    for allocation in 1..engine.memory.next_allocation {
        let facts = engine
            .memory
            .allocations
            .get(&allocation)
            .map(|allocation| (allocation.address_space, allocation.bytes.len()))
            .ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "preexisting allocation remained live",
                    ),
                )
            })?;
        engine.event(
            site,
            SimulationEventKindV1::AllocationPreexisting {
                allocation,
                address_space: facts.0,
                bytes: facts.1,
            },
        )?;
    }
    Ok(())
}

fn initialize_arguments(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    entry: &Function,
    request: &SimulationRequestV1,
) -> Result<Vec<RuntimeValue>, SimulationExecutionErrorV1> {
    let mut parameters = Vec::new();
    parameters
        .try_reserve_exact(request.arguments.len())
        .map_err(|_| engine.fail(SimulationExecutionErrorKindV1::AllocationFailure))?;
    for (index, (argument, ty)) in request
        .arguments
        .iter()
        .zip(&entry.signature.parameters)
        .enumerate()
    {
        let parameter = match (argument, ty) {
            (SimulationArgumentV1::Scalar(value), Type::Scalar(_)) => {
                Ok(RuntimeValue::Scalar(*value))
            }
            (SimulationArgumentV1::Buffer(buffer), Type::Slice(slice)) => {
                let Type::Scalar(element) = slice.element.as_ref() else {
                    return Err(
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted scalar slice",
                        )),
                    );
                };
                let allocation = allocate_argument(engine, index, buffer, AddressSpace::Global)?;
                Ok(RuntimeValue::Slice(SliceValue {
                    allocation,
                    elements: buffer.element_count(engine.target).map_err(|_| {
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted buffer target layout",
                        ))
                    })?,
                    element: *element,
                    address_space: slice.address_space,
                    access: slice.access,
                    byte_offset: 0,
                    byte_len: buffer.bytes().len(),
                }))
            }
            (SimulationArgumentV1::Buffer(buffer), Type::Pointer(pointer)) => {
                let Type::Scalar(element) = pointer.pointee.as_ref() else {
                    return Err(
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted scalar pointer",
                        )),
                    );
                };
                let allocation = allocate_argument(engine, index, buffer, AddressSpace::Global)?;
                Ok(RuntimeValue::Pointer(PointerValue {
                    allocation,
                    byte_offset: 0,
                    element: *element,
                    address_space: pointer.address_space,
                    access: pointer.access,
                    lower_bound: 0,
                    upper_bound: buffer.bytes().len(),
                }))
            }
            (SimulationArgumentV1::BufferView(view), Type::Slice(slice)) => {
                let Type::Scalar(element) = slice.element.as_ref() else {
                    return Err(
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted scalar slice view",
                        )),
                    );
                };
                let allocation = *engine
                    .memory
                    .shared_allocations
                    .get(&view.backing())
                    .ok_or_else(|| {
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted shared backing",
                        ))
                    })?;
                let byte_len = view.byte_len(engine.target).map_err(|_| {
                    engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                        "preflighted shared view layout",
                    ))
                })?;
                Ok(RuntimeValue::Slice(SliceValue {
                    allocation,
                    elements: view.elements(),
                    element: *element,
                    address_space: slice.address_space,
                    access: slice.access,
                    byte_offset: view.byte_offset(),
                    byte_len,
                }))
            }
            (SimulationArgumentV1::BufferView(view), Type::Pointer(pointer)) => {
                let Type::Scalar(element) = pointer.pointee.as_ref() else {
                    return Err(
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted scalar pointer view",
                        )),
                    );
                };
                let allocation = *engine
                    .memory
                    .shared_allocations
                    .get(&view.backing())
                    .ok_or_else(|| {
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted shared backing",
                        ))
                    })?;
                let upper_bound = view
                    .byte_offset()
                    .checked_add(view.byte_len(engine.target).map_err(|_| {
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted shared view layout",
                        ))
                    })?)
                    .ok_or_else(|| {
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted shared view bounds",
                        ))
                    })?;
                Ok(RuntimeValue::Pointer(PointerValue {
                    allocation,
                    byte_offset: view.byte_offset(),
                    element: *element,
                    address_space: pointer.address_space,
                    access: pointer.access,
                    lower_bound: view.byte_offset(),
                    upper_bound,
                }))
            }
            _ => Err(
                engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                    "preflighted argument shape",
                )),
            ),
        }?;
        parameters.push(parameter);
    }
    Ok(parameters)
}

fn initialize_shared_buffers(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    request: &SimulationRequestV1,
) -> Result<(), SimulationExecutionErrorV1> {
    for shared in &request.shared_buffers {
        let bytes = try_clone_slice(shared.buffer.bytes()).map_err(|kind| engine.fail(kind))?;
        let initialized =
            try_clone_slice(shared.buffer.initialized()).map_err(|kind| engine.fail(kind))?;
        let allocation = engine
            .memory
            .allocate(
                AddressSpace::Global,
                shared.buffer.access(),
                shared.buffer.alignment(),
                bytes,
                initialized,
                engine.limits,
            )
            .map_err(|kind| engine.fail(kind))?;
        if engine
            .memory
            .shared_allocations
            .insert(shared.id, allocation)
            .is_some()
        {
            return Err(
                engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                    "preflighted unique shared backing",
                )),
            );
        }
    }
    Ok(())
}

fn allocate_argument(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    index: usize,
    buffer: &BufferArgumentV1,
    address_space: AddressSpace,
) -> Result<u64, SimulationExecutionErrorV1> {
    engine
        .memory
        .validate_allocation(buffer.bytes().len(), engine.limits)
        .map_err(|kind| engine.fail(kind))?;
    let bytes = try_clone_slice(buffer.bytes()).map_err(|kind| engine.fail(kind))?;
    let initialized = try_clone_slice(buffer.initialized()).map_err(|kind| engine.fail(kind))?;
    let id = engine
        .memory
        .allocate(
            address_space,
            buffer.access(),
            buffer.alignment(),
            bytes,
            initialized,
            engine.limits,
        )
        .map_err(|kind| engine.fail(kind))?;
    engine.memory.argument_allocations[index] = Some(id);
    Ok(id)
}

fn copy_back_arguments(
    memory: &Memory,
    source: &[SimulationArgumentV1],
) -> Result<Vec<SimulationArgumentV1>, SimulationExecutionErrorV1> {
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(source.len())
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    for (index, argument) in source.iter().enumerate() {
        let output = match argument {
            SimulationArgumentV1::Scalar(value) => SimulationArgumentV1::Scalar(*value),
            SimulationArgumentV1::Buffer(buffer) => {
                let allocation_id = memory.argument_allocations[index].ok_or_else(|| {
                    top_level_error(SimulationExecutionErrorKindV1::InternalInvariant(
                        "distinct argument allocation",
                    ))
                })?;
                let allocation = memory.allocations.get(&allocation_id).ok_or_else(|| {
                    top_level_error(SimulationExecutionErrorKindV1::DanglingPointer {
                        allocation: allocation_id,
                    })
                })?;
                SimulationArgumentV1::Buffer(buffer.with_contents(
                    try_clone_slice(&allocation.bytes).map_err(top_level_error)?,
                    try_clone_slice(&allocation.initialized).map_err(top_level_error)?,
                ))
            }
            SimulationArgumentV1::BufferView(view) => {
                SimulationArgumentV1::BufferView(view.clone())
            }
        };
        arguments.push(output);
    }
    Ok(arguments)
}

fn copy_back_shared_buffers(
    memory: &Memory,
    source: &[SharedBufferV1],
) -> Result<Vec<SharedBufferV1>, SimulationExecutionErrorV1> {
    let mut outputs = Vec::new();
    outputs
        .try_reserve_exact(source.len())
        .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
    for shared in source {
        let allocation_id = memory.shared_allocations.get(&shared.id).ok_or_else(|| {
            top_level_error(SimulationExecutionErrorKindV1::InternalInvariant(
                "shared output allocation",
            ))
        })?;
        let allocation = memory.allocations.get(allocation_id).ok_or_else(|| {
            top_level_error(SimulationExecutionErrorKindV1::DanglingPointer {
                allocation: *allocation_id,
            })
        })?;
        let buffer = shared.buffer.with_contents(
            try_clone_slice(&allocation.bytes).map_err(top_level_error)?,
            try_clone_slice(&allocation.initialized).map_err(top_level_error)?,
        );
        outputs.push(SharedBufferV1 {
            id: shared.id,
            buffer,
        });
    }
    Ok(outputs)
}

struct RuntimeFrame<'a> {
    function_index: usize,
    function: &'a Function,
    values: HashMap<ValueId, RuntimeValue>,
    allocations: Vec<FrameAllocation>,
    current: BlockId,
    current_index: usize,
    incoming: Vec<RuntimeValue>,
    operation: usize,
    block_entered: bool,
    active_operation: Option<CompactSite>,
}

#[derive(Clone, Copy)]
struct FrameAllocation {
    id: u64,
    lifecycle_observed: bool,
}

enum FrameAction<'a> {
    Continue,
    Call {
        function_index: usize,
        function: &'a Function,
        site: CompactSite,
    },
    Barrier {
        site: CompactSite,
        barrier: &'a WorkgroupBarrier,
    },
    Return,
}

struct InvocationMachine<'a> {
    invocation: SimulationInvocationV1,
    frames: Vec<RuntimeFrame<'a>>,
    active_depth: usize,
    completed: bool,
    waiting: Option<MachineWait<'a>>,
    pending_wave_input: Option<WaveInput>,
}

#[derive(Clone, Copy)]
struct BarrierArrival<'a> {
    site: CompactSite,
    barrier: &'a WorkgroupBarrier,
}

#[derive(Clone, Copy)]
enum WaveInput {
    LaneId,
    Predicate(bool),
    Shuffle {
        value: ScalarBitsV1,
        source_lane: u32,
        tile_width: u32,
    },
}

#[derive(Clone, Copy)]
struct WaveArrival<'a> {
    site: CompactSite,
    wave: &'a WaveOperation,
}

#[derive(Clone, Copy)]
enum MachineWait<'a> {
    Barrier(BarrierArrival<'a>),
    Wave(WaveArrival<'a>),
}

enum MachineYield<'a> {
    Barrier(BarrierArrival<'a>),
    Wave(WaveArrival<'a>),
    Complete,
}

impl<'a> RuntimeFrame<'a> {
    fn new(
        engine: &Engine<'_, impl SimulationEventSinkV1>,
        function_index: usize,
        function: &'a Function,
        arguments: &[RuntimeValue],
    ) -> Result<Self, SimulationExecutionErrorV1> {
        let body = function.body.as_ref().ok_or_else(|| {
            engine.fail(SimulationExecutionErrorKindV1::MissingBody(
                function.id.clone(),
            ))
        })?;
        let entry = body.blocks.first().ok_or_else(|| {
            engine.fail(SimulationExecutionErrorKindV1::MissingBody(
                function.id.clone(),
            ))
        })?;
        if arguments.len() != body.parameters.len() {
            return Err(engine.fail(SimulationExecutionErrorKindV1::ResultArity {
                expected: body.parameters.len(),
                actual: arguments.len(),
            }));
        }
        let mut values = HashMap::new();
        let value_capacity = engine.function_ssa_values[function_index];
        values
            .try_reserve(value_capacity)
            .map_err(|_| engine.fail(SimulationExecutionErrorKindV1::AllocationFailure))?;
        for (id, argument) in body.parameters.iter().copied().zip(arguments) {
            bind_runtime_value(engine, &mut values, id, argument.clone())?;
        }
        Ok(Self {
            function_index,
            function,
            values,
            allocations: Vec::new(),
            current: entry.id,
            current_index: 0,
            incoming: Vec::new(),
            operation: 0,
            block_entered: false,
            active_operation: None,
        })
    }

    fn reset(
        &mut self,
        engine: &Engine<'_, impl SimulationEventSinkV1>,
        function_index: usize,
        function: &'a Function,
        arguments: &[RuntimeValue],
    ) -> Result<(), SimulationExecutionErrorV1> {
        let body = function.body.as_ref().ok_or_else(|| {
            engine.fail(SimulationExecutionErrorKindV1::MissingBody(
                function.id.clone(),
            ))
        })?;
        let entry = body.blocks.first().ok_or_else(|| {
            engine.fail(SimulationExecutionErrorKindV1::MissingBody(
                function.id.clone(),
            ))
        })?;
        if arguments.len() != body.parameters.len() {
            return Err(engine.fail(SimulationExecutionErrorKindV1::ResultArity {
                expected: body.parameters.len(),
                actual: arguments.len(),
            }));
        }
        if !self.allocations.is_empty() || self.active_operation.is_some() {
            return Err(
                engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                    "completed root frame retained live state",
                )),
            );
        }
        self.values.clear();
        let value_capacity = engine.function_ssa_values[function_index];
        if self.values.capacity() < value_capacity {
            self.values
                .try_reserve(value_capacity)
                .map_err(|_| engine.fail(SimulationExecutionErrorKindV1::AllocationFailure))?;
        }
        for (id, argument) in body.parameters.iter().copied().zip(arguments) {
            bind_runtime_value(engine, &mut self.values, id, argument.clone())?;
        }
        self.function_index = function_index;
        self.function = function;
        self.current = entry.id;
        self.current_index = 0;
        self.incoming.clear();
        self.operation = 0;
        self.block_entered = false;
        Ok(())
    }
}

fn function_ssa_definition_count(function: &Function) -> Option<usize> {
    let body = function.body.as_ref()?;
    let mut count = body.parameters.len();
    for block in &body.blocks {
        count = count.checked_add(block.parameters.len())?;
        for operation in &block.operations {
            count = count.checked_add(operation.results.len())?;
        }
    }
    Some(count)
}

impl<'a> InvocationMachine<'a> {
    fn new(
        engine: &Engine<'_, impl SimulationEventSinkV1>,
        invocation: SimulationInvocationV1,
        function_index: usize,
        function: &'a Function,
        arguments: &[RuntimeValue],
    ) -> Result<Self, SimulationExecutionErrorV1> {
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(1)
            .map_err(|_| engine.fail(SimulationExecutionErrorKindV1::AllocationFailure))?;
        frames.push(RuntimeFrame::new(
            engine,
            function_index,
            function,
            arguments,
        )?);
        Ok(Self {
            invocation,
            frames,
            active_depth: 1,
            completed: false,
            waiting: None,
            pending_wave_input: None,
        })
    }

    fn advance_until_yield<S: SimulationEventSinkV1>(
        &mut self,
        engine: &mut Engine<'a, S>,
        phase: u64,
    ) -> Result<MachineYield<'a>, SimulationExecutionErrorV1> {
        if self.completed || self.active_depth == 0 || self.waiting.is_some() {
            return Err(
                engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                    "advanced completed invocation machine",
                )),
            );
        }

        loop {
            let checkpoint_site = self.frames.get(self.active_depth - 1).and_then(|frame| {
                if !frame.block_entered {
                    return None;
                }
                let block = frame
                    .function
                    .body
                    .as_ref()?
                    .blocks
                    .get(frame.current_index)?;
                block
                    .operations
                    .get(frame.operation)
                    .map(|_| operation_site(frame.function_index, block, frame.operation))
            });
            if let Some(site) = checkpoint_site {
                engine.debug_checkpoint(
                    &self.frames[..self.active_depth],
                    site,
                    SimulationDebugCheckpointPhaseV1::BeforeOperation,
                );
            }
            if self.frames[self.active_depth - 1].block_entered
                && self.frames[self.active_depth - 1]
                    .function
                    .body
                    .as_ref()
                    .and_then(|body| {
                        body.blocks
                            .get(self.frames[self.active_depth - 1].current_index)
                    })
                    .and_then(|block| {
                        block
                            .operations
                            .get(self.frames[self.active_depth - 1].operation)
                    })
                    .is_some_and(|operation| matches!(operation.kind, OperationKind::Wave(_)))
            {
                let frame = self.frames.get_mut(self.active_depth - 1).ok_or_else(|| {
                    engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                        "wave runtime frame",
                    ))
                })?;
                let arrival = advance_wave_frame(engine, frame, &mut self.pending_wave_input)?;
                return Ok(MachineYield::Wave(arrival));
            }
            let action = {
                let frame = self.frames.get_mut(self.active_depth - 1).ok_or_else(|| {
                    engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                        "runtime frame stack",
                    ))
                })?;
                if !frame.block_entered {
                    advance_block_entry_frame(engine, frame)
                } else if is_internal_call_frame(engine, frame) {
                    advance_internal_call_frame(engine, frame)
                } else if is_return_frame(frame) {
                    advance_return_frame(engine, frame)
                } else {
                    advance_frame(engine, frame, phase)
                }
            };
            let action = match action {
                Ok(action) => action,
                Err(mut error) => {
                    self.frames.truncate(self.active_depth);
                    unwind_frames(engine, &mut self.frames, &mut error);
                    self.active_depth = 0;
                    return Err(error);
                }
            };
            if let Some(site) = checkpoint_site
                && matches!(&action, FrameAction::Continue | FrameAction::Barrier { .. })
            {
                engine.debug_checkpoint(
                    &self.frames[..self.active_depth],
                    site,
                    SimulationDebugCheckpointPhaseV1::AfterOperation,
                );
            }
            match action {
                FrameAction::Continue => {}
                FrameAction::Barrier { site, barrier } => {
                    return Ok(MachineYield::Barrier(BarrierArrival { site, barrier }));
                }
                FrameAction::Call {
                    function_index,
                    function,
                    site,
                } => {
                    if self.active_depth == engine.limits.max_call_depth {
                        let mut error = engine.at(
                            site,
                            SimulationExecutionErrorKindV1::CallDepthLimit {
                                limit: engine.limits.max_call_depth,
                            },
                        );
                        self.frames.truncate(self.active_depth);
                        unwind_frames(engine, &mut self.frames, &mut error);
                        self.active_depth = 0;
                        return Err(error);
                    }
                    let arguments =
                        std::mem::take(&mut self.frames[self.active_depth - 1].incoming);
                    let frame_result = if self.active_depth == self.frames.len() {
                        if self.frames.len() == self.frames.capacity()
                            && self.frames.try_reserve_exact(1).is_err()
                        {
                            Err(engine.at(site, SimulationExecutionErrorKindV1::AllocationFailure))
                        } else {
                            RuntimeFrame::new(engine, function_index, function, &arguments)
                                .map(|frame| self.frames.push(frame))
                        }
                    } else {
                        self.frames[self.active_depth].reset(
                            engine,
                            function_index,
                            function,
                            &arguments,
                        )
                    };
                    self.frames[self.active_depth - 1].incoming = arguments;
                    self.frames[self.active_depth - 1].incoming.clear();
                    if let Err(mut error) = frame_result {
                        self.frames.truncate(self.active_depth);
                        unwind_frames(engine, &mut self.frames, &mut error);
                        self.active_depth = 0;
                        return Err(error);
                    }
                    self.active_depth += 1;
                }
                FrameAction::Return => {
                    let returned = std::mem::take(&mut self.frames[self.active_depth - 1].incoming);
                    if self.active_depth == 1 {
                        if !returned.is_empty() {
                            let mut error =
                                engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                                    "kernel returned values after verification",
                                ));
                            self.frames[0].incoming = returned;
                            unwind_frames(engine, &mut self.frames, &mut error);
                            self.active_depth = 0;
                            return Err(error);
                        }
                        self.frames[0].incoming = returned;
                        self.active_depth = 0;
                        self.completed = true;
                        return Ok(MachineYield::Complete);
                    }
                    self.active_depth -= 1;
                    let caller = self.frames.get_mut(self.active_depth - 1).ok_or_else(|| {
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "caller frame after nested return",
                        ))
                    })?;
                    let site = caller.active_operation.ok_or_else(|| {
                        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                            "caller operation lifecycle",
                        ))
                    })?;
                    let operation = caller
                        .function
                        .body
                        .as_ref()
                        .and_then(|body| body.blocks.get(caller.current_index))
                        .and_then(|block| block.operations.get(caller.operation))
                        .ok_or_else(|| {
                            engine.at(
                                site,
                                SimulationExecutionErrorKindV1::InternalInvariant(
                                    "suspended call operation",
                                ),
                            )
                        })?;
                    if let Err(mut error) = bind_dynamic_results(
                        engine,
                        &mut caller.values,
                        &operation.results,
                        &returned,
                        &site,
                    ) {
                        self.frames[self.active_depth].incoming = returned;
                        self.frames[self.active_depth].incoming.clear();
                        self.frames.truncate(self.active_depth);
                        unwind_frames(engine, &mut self.frames, &mut error);
                        self.active_depth = 0;
                        return Err(error);
                    }
                    caller.incoming.clear();
                    caller.active_operation = None;
                    if let Err(mut error) = engine.end_lifecycle(
                        &site,
                        SimulationEventKindV1::OperationEnd {
                            outcome: SimulationExecutionOutcomeV1::Completed,
                        },
                    ) {
                        self.frames[self.active_depth].incoming = returned;
                        self.frames[self.active_depth].incoming.clear();
                        self.frames.truncate(self.active_depth);
                        unwind_frames(engine, &mut self.frames, &mut error);
                        self.active_depth = 0;
                        return Err(error);
                    }
                    caller.operation += 1;
                    self.frames[self.active_depth].incoming = returned;
                    self.frames[self.active_depth].incoming.clear();
                    engine.debug_checkpoint(
                        &self.frames[..self.active_depth],
                        site,
                        SimulationDebugCheckpointPhaseV1::AfterOperation,
                    );
                }
            }
        }
    }

    fn complete_wave<S: SimulationEventSinkV1>(
        &mut self,
        engine: &mut Engine<'a, S>,
        value: ScalarBitsV1,
    ) -> Result<(), SimulationExecutionErrorV1> {
        let Some(MachineWait::Wave(arrival)) = self.waiting.take() else {
            return Err(
                engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                    "completed a machine not waiting at a wave operation",
                )),
            );
        };
        let frame = self.frames.get_mut(self.active_depth - 1).ok_or_else(|| {
            engine.at(
                arrival.site,
                SimulationExecutionErrorKindV1::InternalInvariant("wave runtime frame"),
            )
        })?;
        let operation = frame
            .function
            .body
            .as_ref()
            .and_then(|body| body.blocks.get(frame.current_index))
            .and_then(|block| block.operations.get(frame.operation))
            .ok_or_else(|| {
                engine.at(
                    arrival.site,
                    SimulationExecutionErrorKindV1::InternalInvariant("suspended wave operation"),
                )
            })?;
        bind_small_results(
            engine,
            &mut frame.values,
            &operation.results,
            SmallResults::One(RuntimeValue::Scalar(value)),
            &arrival.site,
        )?;
        self.pending_wave_input = None;
        frame.active_operation = None;
        engine.end_lifecycle(
            &arrival.site,
            SimulationEventKindV1::OperationEnd {
                outcome: SimulationExecutionOutcomeV1::Completed,
            },
        )?;
        frame.operation += 1;
        engine.debug_checkpoint(
            &self.frames[..self.active_depth],
            arrival.site,
            SimulationDebugCheckpointPhaseV1::AfterOperation,
        );
        Ok(())
    }

    fn wave_input(&self) -> Option<WaveInput> {
        self.pending_wave_input
    }
}

#[inline(never)]
fn advance_block_entry_frame<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    frame: &mut RuntimeFrame<'a>,
) -> Result<FrameAction<'a>, SimulationExecutionErrorV1> {
    let block = frame
        .function
        .body
        .as_ref()
        .and_then(|body| body.blocks.get(frame.current_index))
        .ok_or_else(|| engine.fail(SimulationExecutionErrorKindV1::UnknownBlock(frame.current)))?;
    bind_block_arguments(
        engine,
        frame.function_index,
        block,
        &frame.incoming,
        &mut frame.values,
    )?;
    frame.incoming.clear();
    let site = terminator_site(frame.function_index, block);
    engine.event(&site, SimulationEventKindV1::BlockEnter)?;
    frame.block_entered = true;
    Ok(FrameAction::Continue)
}

fn is_internal_call_frame(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    frame: &RuntimeFrame<'_>,
) -> bool {
    if !frame.block_entered {
        return false;
    }
    let Some(block) = frame
        .function
        .body
        .as_ref()
        .and_then(|body| body.blocks.get(frame.current_index))
    else {
        return false;
    };
    if !matches!(
        block.operations.get(frame.operation),
        Some(Operation {
            kind: OperationKind::Call { .. },
            ..
        })
    ) {
        return false;
    }
    matches!(
        engine
            .call_targets
            .get(frame.function_index)
            .and_then(|function| function.get(frame.current_index))
            .and_then(|block| block.get(frame.operation)),
        Some(CallTarget::Internal(_))
    )
}

fn is_return_frame(frame: &RuntimeFrame<'_>) -> bool {
    let Some(block) = frame
        .function
        .body
        .as_ref()
        .and_then(|body| body.blocks.get(frame.current_index))
    else {
        return false;
    };
    frame.operation == block.operations.len()
        && matches!(block.terminator, Some(Terminator::Return { .. }))
}

#[inline(never)]
fn advance_return_frame<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    frame: &mut RuntimeFrame<'a>,
) -> Result<FrameAction<'a>, SimulationExecutionErrorV1> {
    let block = frame
        .function
        .body
        .as_ref()
        .and_then(|body| body.blocks.get(frame.current_index))
        .ok_or_else(|| engine.fail(SimulationExecutionErrorKindV1::UnknownBlock(frame.current)))?;
    let site = terminator_site(frame.function_index, block);
    let Some(Terminator::Return { values }) = &block.terminator else {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant("return frame dispatch"),
        ));
    };
    engine.step(&site)?;
    engine.event(&site, SimulationEventKindV1::Terminator)?;
    resolve_values_into(engine, &frame.values, values, &site, &mut frame.incoming)?;
    release_frame_allocations_observed(engine, &mut frame.allocations, &site)?;
    engine.event(&site, SimulationEventKindV1::Return)?;
    Ok(FrameAction::Return)
}

#[inline(never)]
fn advance_internal_call_frame<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    frame: &mut RuntimeFrame<'a>,
) -> Result<FrameAction<'a>, SimulationExecutionErrorV1> {
    let block = frame
        .function
        .body
        .as_ref()
        .and_then(|body| body.blocks.get(frame.current_index))
        .ok_or_else(|| engine.fail(SimulationExecutionErrorKindV1::UnknownBlock(frame.current)))?;
    let operation = block.operations.get(frame.operation).ok_or_else(|| {
        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
            "internal call operation position",
        ))
    })?;
    let OperationKind::Call { arguments, .. } = &operation.kind else {
        return Err(
            engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                "internal call operation dispatch",
            )),
        );
    };
    let site = operation_site(frame.function_index, block, frame.operation);
    engine.step(&site)?;
    engine.begin_lifecycle(&site, SimulationEventKindV1::OperationBegin)?;
    frame.active_operation = Some(site);
    let CallTarget::Internal(callee_index) =
        engine.call_targets[frame.function_index][frame.current_index][frame.operation]
    else {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant("preflighted internal call index"),
        ));
    };
    let callee_function = &engine.module.functions[engine.function_module_indices[callee_index]];
    if callee_function.role != FunctionRole::InternalHelper {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant("preflighted internal call"),
        ));
    }
    resolve_values_into(engine, &frame.values, arguments, &site, &mut frame.incoming)?;
    engine.call_event(&site, callee_index)?;
    Ok(FrameAction::Call {
        function_index: callee_index,
        function: callee_function,
        site,
    })
}

fn advance_frame<'a, S: SimulationEventSinkV1>(
    engine: &mut Engine<'a, S>,
    frame: &mut RuntimeFrame<'a>,
    phase: u64,
) -> Result<FrameAction<'a>, SimulationExecutionErrorV1> {
    let body = frame.function.body.as_ref().ok_or_else(|| {
        engine.fail(SimulationExecutionErrorKindV1::MissingBody(
            frame.function.id.clone(),
        ))
    })?;
    let block = body
        .blocks
        .get(frame.current_index)
        .ok_or_else(|| engine.fail(SimulationExecutionErrorKindV1::UnknownBlock(frame.current)))?;
    if !frame.block_entered {
        bind_block_arguments(
            engine,
            frame.function_index,
            block,
            &frame.incoming,
            &mut frame.values,
        )?;
        frame.incoming.clear();
        let site = terminator_site(frame.function_index, block);
        engine.event(&site, SimulationEventKindV1::BlockEnter)?;
        frame.block_entered = true;
        return Ok(FrameAction::Continue);
    }

    if let Some(operation) = block.operations.get(frame.operation) {
        let site = operation_site(frame.function_index, block, frame.operation);
        engine.step(&site)?;
        engine.begin_lifecycle(&site, SimulationEventKindV1::OperationBegin)?;
        frame.active_operation = Some(site);
        if let OperationKind::Call { arguments, .. } = &operation.kind {
            let target =
                engine.call_targets[frame.function_index][frame.current_index][frame.operation];
            let callee_index = match target {
                CallTarget::Float(_) => None,
                CallTarget::Internal(callee_index) => Some(callee_index),
                CallTarget::NotCall => {
                    return Err(engine.at(
                        site,
                        SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted operation call index",
                        ),
                    ));
                }
            };
            if let Some(callee_index) = callee_index {
                let callee_function =
                    &engine.module.functions[engine.function_module_indices[callee_index]];
                if callee_function.role != FunctionRole::InternalHelper {
                    return Err(engine.at(
                        site,
                        SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted internal call",
                        ),
                    ));
                }
                resolve_values_into(engine, &frame.values, arguments, &site, &mut frame.incoming)?;
                engine.call_event(&site, callee_index)?;
                return Ok(FrameAction::Call {
                    function_index: callee_index,
                    function: callee_function,
                    site,
                });
            }
        }
        if let OperationKind::WorkgroupBarrier(barrier) = &operation.kind {
            engine.event(
                &site,
                SimulationEventKindV1::WorkgroupBarrierArrive { phase },
            )?;
            engine.debug_barrier(site, SimulationDebugBarrierActionV1::Arrive, phase, 1);
            frame.active_operation = None;
            engine.end_lifecycle(
                &site,
                SimulationEventKindV1::OperationEnd {
                    outcome: SimulationExecutionOutcomeV1::Completed,
                },
            )?;
            frame.operation += 1;
            return Ok(FrameAction::Barrier { site, barrier });
        }
        return advance_non_control_operation(engine, frame, block, operation, site);
    }

    let site = terminator_site(frame.function_index, block);
    let terminator = block.terminator.as_ref().ok_or_else(|| {
        engine.at(
            site,
            SimulationExecutionErrorKindV1::MissingTerminator(block.id),
        )
    })?;
    engine.step(&site)?;
    engine.event(&site, SimulationEventKindV1::Terminator)?;
    if let Terminator::Return { values } = terminator {
        resolve_values_into(engine, &frame.values, values, &site, &mut frame.incoming)?;
        release_frame_allocations_observed(engine, &mut frame.allocations, &site)?;
        engine.event(&site, SimulationEventKindV1::Return)?;
        return Ok(FrameAction::Return);
    }
    advance_non_return_terminator(engine, frame, terminator, site)
}

#[inline(never)]
fn advance_wave_frame<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    frame: &mut RuntimeFrame<'a>,
    pending: &mut Option<WaveInput>,
) -> Result<WaveArrival<'a>, SimulationExecutionErrorV1> {
    let block = frame
        .function
        .body
        .as_ref()
        .and_then(|body| body.blocks.get(frame.current_index))
        .ok_or_else(|| engine.fail(SimulationExecutionErrorKindV1::UnknownBlock(frame.current)))?;
    let operation = block.operations.get(frame.operation).ok_or_else(|| {
        engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
            "wave operation position",
        ))
    })?;
    let OperationKind::Wave(wave) = &operation.kind else {
        return Err(
            engine.fail(SimulationExecutionErrorKindV1::InternalInvariant(
                "wave operation dispatch",
            )),
        );
    };
    let site = operation_site(frame.function_index, block, frame.operation);
    engine.step(&site)?;
    engine.begin_lifecycle(&site, SimulationEventKindV1::OperationBegin)?;
    frame.active_operation = Some(site);
    prepare_wave_wait(engine, frame, wave, site, pending)?;
    Ok(WaveArrival { site, wave })
}

#[inline(never)]
fn prepare_wave_wait(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    frame: &RuntimeFrame<'_>,
    wave: &WaveOperation,
    site: CompactSite,
    pending: &mut Option<WaveInput>,
) -> Result<(), SimulationExecutionErrorV1> {
    *pending = Some(match wave.kind {
        WaveOperationKind::LaneId => WaveInput::LaneId,
        WaveOperationKind::Ballot { predicate }
        | WaveOperationKind::Any { predicate }
        | WaveOperationKind::All { predicate } => {
            let predicate = scalar_value(engine, &frame.values, predicate, &site)?
                .as_bool()
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::RuntimeType {
                            value: Some(predicate),
                            expected: "boolean wave predicate",
                        },
                    )
                })?;
            WaveInput::Predicate(predicate)
        }
        WaveOperationKind::ShuffleIndex {
            value,
            source_lane,
            tile_width,
        } => {
            let value = scalar_value(engine, &frame.values, value, &site)?;
            let source = scalar_value(engine, &frame.values, source_lane, &site)?;
            if source.ty() != ScalarType::U32 {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(source_lane),
                        expected: "u32 wave shuffle source lane",
                    },
                ));
            }
            let source_lane = source.bits() as u32;
            if source_lane >= tile_width {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::WaveShuffleSourceOutOfRange {
                        source_lane,
                        tile_width,
                    },
                ));
            }
            WaveInput::Shuffle {
                value,
                source_lane,
                tile_width,
            }
        }
        WaveOperationKind::ReduceF32 { .. } | WaveOperationKind::BroadcastF32 { .. } => {
            return Err(engine.at(
                site,
                SimulationExecutionErrorKindV1::InternalInvariant(
                    "unsupported wave operation passed preflight",
                ),
            ));
        }
    });
    Ok(())
}

#[inline(never)]
fn advance_non_return_terminator<'a>(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    frame: &mut RuntimeFrame<'a>,
    terminator: &Terminator,
    site: CompactSite,
) -> Result<FrameAction<'a>, SimulationExecutionErrorV1> {
    match terminator {
        Terminator::Branch { target, arguments } => {
            resolve_values_into(engine, &frame.values, arguments, &site, &mut frame.incoming)?;
            engine.event(&site, SimulationEventKindV1::Branch { target: *target })?;
            branch_to(engine, frame, *target)
        }
        Terminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            let take_then = scalar_value(engine, &frame.values, *condition, &site)?
                .as_bool()
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::RuntimeType {
                            value: Some(*condition),
                            expected: "boolean branch condition",
                        },
                    )
                })?;
            let (target, arguments) = if take_then {
                (then_target, then_arguments)
            } else {
                (else_target, else_arguments)
            };
            resolve_values_into(engine, &frame.values, arguments, &site, &mut frame.incoming)?;
            engine.event(&site, SimulationEventKindV1::Branch { target: *target })?;
            branch_to(engine, frame, *target)
        }
        Terminator::Switch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            let selector = scalar_value(engine, &frame.values, *selector, &site)?.bits();
            let SwitchLookup::Index(lookup) =
                &engine.switch_targets[frame.function_index][frame.current_index]
            else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "preflighted index switch lookup",
                    ),
                ));
            };
            let selected = lookup.get(&selector).and_then(|index| cases.get(*index));
            let (target, arguments) = selected
                .map(|case| (&case.target, &case.arguments))
                .unwrap_or((default_target, default_arguments));
            resolve_values_into(engine, &frame.values, arguments, &site, &mut frame.incoming)?;
            engine.event(&site, SimulationEventKindV1::Branch { target: *target })?;
            branch_to(engine, frame, *target)
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            let selector = scalar_value(engine, &frame.values, *selector, &site)?;
            let SwitchLookup::Integer(lookup) =
                &engine.switch_targets[frame.function_index][frame.current_index]
            else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "preflighted integer switch lookup",
                    ),
                ));
            };
            let selected = lookup
                .get(&(selector.ty(), selector.bits()))
                .and_then(|index| cases.get(*index));
            let (target, arguments) = selected
                .map(|case| (&case.target, &case.arguments))
                .unwrap_or((default_target, default_arguments));
            resolve_values_into(engine, &frame.values, arguments, &site, &mut frame.incoming)?;
            engine.event(&site, SimulationEventKindV1::Branch { target: *target })?;
            branch_to(engine, frame, *target)
        }
        Terminator::Return { .. } => Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "return reached non-return terminator evaluator",
            ),
        )),
        Terminator::Unreachable => {
            Err(engine.at(site, SimulationExecutionErrorKindV1::ReachedUnreachable))
        }
    }
}

#[inline(never)]
fn advance_non_control_operation<'a>(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    frame: &mut RuntimeFrame<'a>,
    block: &BasicBlock,
    operation: &Operation,
    site: CompactSite,
) -> Result<FrameAction<'a>, SimulationExecutionErrorV1> {
    let results = execute_operation(
        engine,
        frame.function_index,
        block,
        frame.operation,
        operation,
        &frame.values,
        &mut frame.allocations,
    )?;
    bind_small_results(
        engine,
        &mut frame.values,
        &operation.results,
        results,
        &site,
    )?;
    frame.active_operation = None;
    engine.end_lifecycle(
        &site,
        SimulationEventKindV1::OperationEnd {
            outcome: SimulationExecutionOutcomeV1::Completed,
        },
    )?;
    frame.operation += 1;
    Ok(FrameAction::Continue)
}

fn branch_to<'a>(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    frame: &mut RuntimeFrame<'a>,
    target: BlockId,
) -> Result<FrameAction<'a>, SimulationExecutionErrorV1> {
    let target_index = engine.block_indices[frame.function_index]
        .get(&target)
        .copied()
        .ok_or_else(|| engine.fail(SimulationExecutionErrorKindV1::UnknownBlock(target)))?;
    frame.current = target;
    frame.current_index = target_index;
    frame.operation = 0;
    frame.block_entered = false;
    Ok(FrameAction::Continue)
}

fn release_frame_allocations_observed(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    allocations: &mut Vec<FrameAllocation>,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    while let Some(allocation) = allocations.pop() {
        let released = engine
            .memory
            .release_one(allocation.id)
            .map_err(|kind| engine.at(*site, kind))?;
        if !released {
            return Err(engine.at(
                *site,
                SimulationExecutionErrorKindV1::InternalInvariant(
                    "frame allocation remained live until release",
                ),
            ));
        }
        if allocation.lifecycle_observed {
            engine.end_lifecycle(
                site,
                SimulationEventKindV1::AllocationReleased {
                    allocation: allocation.id,
                },
            )?;
        }
    }
    Ok(())
}

fn unwind_frames(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    frames: &mut Vec<RuntimeFrame<'_>>,
    primary: &mut SimulationExecutionErrorV1,
) {
    while let Some(mut frame) = frames.pop() {
        let site = frame.active_operation.unwrap_or(CompactSite {
            function: frame.function_index,
            block: frame.current,
            operation: None,
        });
        while !frame.allocations.is_empty() {
            if let Err(secondary) =
                release_frame_allocations_observed(engine, &mut frame.allocations, &site)
            {
                attach_observation_failure(primary, secondary);
            }
        }
        if let Some(operation_site) = frame.active_operation
            && let Err(secondary) = engine.end_lifecycle(
                &operation_site,
                SimulationEventKindV1::OperationEnd {
                    outcome: SimulationExecutionOutcomeV1::Failed,
                },
            )
        {
            attach_observation_failure(primary, secondary);
        }
    }
}

fn attach_observation_failure(
    primary: &mut SimulationExecutionErrorV1,
    secondary: SimulationExecutionErrorV1,
) {
    if primary.observation_failure.is_none() {
        primary.observation_failure = Some(SimulationObservationFailureV1 {
            invocation: secondary.invocation,
            site: secondary.site,
            kind: secondary.kind,
        });
    }
}

enum SmallResults<T> {
    None,
    One(T),
    Two(T, T),
}

impl<T> SmallResults<T> {
    const fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::One(_) => 1,
            Self::Two(_, _) => 2,
        }
    }
}

fn bind_small_results(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    definitions: &[ValueDef],
    results: SmallResults<RuntimeValue>,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    if definitions.len() != results.len() {
        return Err(engine.at(
            *site,
            SimulationExecutionErrorKindV1::ResultArity {
                expected: definitions.len(),
                actual: results.len(),
            },
        ));
    }
    match (definitions, results) {
        ([], SmallResults::None) => Ok(()),
        ([definition], SmallResults::One(value)) => {
            bind_typed_value(engine, values, definition, value, site)
        }
        ([first, second], SmallResults::Two(first_value, second_value)) => {
            bind_typed_value(engine, values, first, first_value, site)?;
            bind_typed_value(engine, values, second, second_value, site)
        }
        _ => Err(engine.at(
            *site,
            SimulationExecutionErrorKindV1::InternalInvariant("small result arity dispatch"),
        )),
    }
}

fn bind_dynamic_results(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    definitions: &[ValueDef],
    results: &[RuntimeValue],
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    if definitions.len() != results.len() {
        return Err(engine.at(
            *site,
            SimulationExecutionErrorKindV1::ResultArity {
                expected: definitions.len(),
                actual: results.len(),
            },
        ));
    }
    for (definition, value) in definitions.iter().zip(results) {
        bind_typed_value(engine, values, definition, value.clone(), site)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn execute_operation(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    function_index: usize,
    block: &BasicBlock,
    ordinal: usize,
    operation: &Operation,
    values: &HashMap<ValueId, RuntimeValue>,
    frame_allocations: &mut Vec<FrameAllocation>,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    let site = operation_site(function_index, block, ordinal);
    let one = |value| Ok(SmallResults::One(value));
    match &operation.kind {
        OperationKind::Constant(constant) => one(RuntimeValue::Scalar(
            constant_scalar(constant, engine.target).map_err(|kind| engine.at(site, kind))?,
        )),
        OperationKind::Intrinsic(intrinsic) => {
            let invocation = engine.invocation.ok_or_else(|| {
                engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant("intrinsic invocation"),
                )
            })?;
            let value = intrinsic_value(intrinsic.kind, invocation);
            one(RuntimeValue::Scalar(
                ScalarBitsV1::index(value, engine.target).map_err(|_| {
                    engine.at(site, SimulationExecutionErrorKindV1::IntegerOutOfRange)
                })?,
            ))
        }
        OperationKind::Unary { op, operand } => {
            let value = scalar_value(engine, values, *operand, &site)?;
            one(RuntimeValue::Scalar(
                execute_unary(*op, value, engine.target).map_err(|kind| engine.at(site, kind))?,
            ))
        }
        OperationKind::Binary { op, lhs, rhs } => {
            let lhs = scalar_value(engine, values, *lhs, &site)?;
            let rhs = scalar_value(engine, values, *rhs, &site)?;
            let scalars = execute_binary(*op, lhs, rhs, engine.target)
                .map_err(|kind| engine.at(site, kind))?;
            Ok(match scalars {
                SmallResults::None => SmallResults::None,
                SmallResults::One(value) => SmallResults::One(RuntimeValue::Scalar(value)),
                SmallResults::Two(first, second) => {
                    SmallResults::Two(RuntimeValue::Scalar(first), RuntimeValue::Scalar(second))
                }
            })
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            let lhs = scalar_value(engine, values, *lhs, &site)?;
            let rhs = scalar_value(engine, values, *rhs, &site)?;
            one(RuntimeValue::Scalar(ScalarBitsV1::boolean(
                execute_compare(*predicate, lhs, rhs, engine.target)
                    .map_err(|kind| engine.at(site, kind))?,
            )))
        }
        OperationKind::Cast { kind, value, to } => {
            let value = scalar_value(engine, values, *value, &site)?;
            let Type::Scalar(to) = to else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant("preflighted scalar cast"),
                ));
            };
            one(RuntimeValue::Scalar(
                execute_cast(*kind, value, *to, engine.target)
                    .map_err(|kind| engine.at(site, kind))?,
            ))
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => {
            let condition = scalar_value(engine, values, *condition, &site)?
                .as_bool()
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::RuntimeType {
                            value: Some(*condition),
                            expected: "boolean select condition",
                        },
                    )
                })?;
            one(runtime_value(
                engine,
                values,
                if condition { *true_value } else { *false_value },
                &site,
            )?
            .clone())
        }
        OperationKind::Call {
            callee: _,
            arguments,
        } => {
            let block_index = engine.block_indices[function_index]
                .get(&block.id)
                .copied()
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted operation block index",
                        ),
                    )
                })?;
            let CallTarget::Float(operation) =
                engine.call_targets[function_index][block_index][ordinal]
            else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "non-float call reached non-recursive operation evaluator",
                    ),
                ));
            };
            execute_float_call(engine, values, operation, arguments, site)
        }
        OperationKind::Alloca {
            element,
            count,
            address_space,
            alignment,
        } => {
            let Type::Scalar(element) = element else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "preflighted scalar allocation",
                    ),
                ));
            };
            if *address_space != AddressSpace::Private {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "preflighted private allocation",
                    ),
                ));
            }
            let count = match count {
                Some(count) => scalar_nonnegative_usize(
                    scalar_value(engine, values, *count, &site)?,
                    engine.target,
                )
                .map_err(|kind| engine.at(site, kind))?,
                None => 1,
            };
            let element_bytes = engine.target.scalar_bytes(*element).ok_or_else(|| {
                engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "preflighted allocation element",
                    ),
                )
            })?;
            let bytes = count.checked_mul(element_bytes).ok_or_else(|| {
                engine.at(
                    site,
                    SimulationExecutionErrorKindV1::AllocationBytesLimit {
                        actual: usize::MAX,
                        limit: engine.limits.max_allocation_bytes,
                    },
                )
            })?;
            engine
                .memory
                .validate_allocation(bytes, engine.limits)
                .map_err(|kind| engine.at(site, kind))?;
            let allocation_bytes = try_filled(bytes, 0_u8).map_err(|kind| engine.at(site, kind))?;
            let initialized = try_filled(bytes, false).map_err(|kind| engine.at(site, kind))?;
            frame_allocations
                .try_reserve(1)
                .map_err(|_| engine.at(site, SimulationExecutionErrorKindV1::AllocationFailure))?;
            let reserved = engine.reserve_event_closure(&site)?;
            let id = match engine.memory.allocate(
                AddressSpace::Private,
                AccessMode::ReadWrite,
                *alignment,
                allocation_bytes,
                initialized,
                engine.limits,
            ) {
                Ok(id) => id,
                Err(kind) => {
                    engine.cancel_event_closure(reserved);
                    return Err(engine.at(site, kind));
                }
            };
            frame_allocations.push(FrameAllocation {
                id,
                lifecycle_observed: reserved,
            });
            engine.emit_reserved_begin(
                &site,
                SimulationEventKindV1::AllocationCreated {
                    allocation: id,
                    address_space: AddressSpace::Private,
                    bytes,
                },
                reserved,
            )?;
            one(RuntimeValue::Pointer(PointerValue {
                allocation: id,
                byte_offset: 0,
                element: *element,
                address_space: AddressSpace::Private,
                access: AccessMode::ReadWrite,
                lower_bound: 0,
                upper_bound: bytes,
            }))
        }
        OperationKind::SliceLength { slice } => {
            let RuntimeValue::Slice(slice) = runtime_value(engine, values, *slice, &site)? else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*slice),
                        expected: "slice",
                    },
                ));
            };
            one(RuntimeValue::Scalar(
                ScalarBitsV1::index(slice.elements as u64, engine.target).map_err(|_| {
                    engine.at(site, SimulationExecutionErrorKindV1::IntegerOutOfRange)
                })?,
            ))
        }
        OperationKind::SliceData { slice } => {
            let RuntimeValue::Slice(slice) = runtime_value(engine, values, *slice, &site)? else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*slice),
                        expected: "slice",
                    },
                ));
            };
            let upper_bound = slice
                .byte_offset
                .checked_add(slice.byte_len)
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted slice view bounds",
                        ),
                    )
                })?;
            one(RuntimeValue::Pointer(PointerValue {
                allocation: slice.allocation,
                byte_offset: slice.byte_offset,
                element: slice.element,
                address_space: slice.address_space,
                access: slice.access,
                lower_bound: slice.byte_offset,
                upper_bound,
            }))
        }
        OperationKind::GetElementPointer { base, offset } => {
            let RuntimeValue::Pointer(pointer) = runtime_value(engine, values, *base, &site)?
            else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*base),
                        expected: "pointer",
                    },
                ));
            };
            let offset = scalar_nonnegative_usize(
                scalar_value(engine, values, *offset, &site)?,
                engine.target,
            )
            .map_err(|kind| engine.at(site, kind))?;
            let element_bytes = engine.target.scalar_bytes(pointer.element).ok_or_else(|| {
                engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "preflighted pointer element",
                    ),
                )
            })?;
            let byte_delta = offset.checked_mul(element_bytes).ok_or_else(|| {
                engine.at(site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
            })?;
            let byte_offset = pointer.byte_offset.checked_add(byte_delta).ok_or_else(|| {
                engine.at(site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
            })?;
            one(RuntimeValue::Pointer(PointerValue {
                byte_offset,
                ..pointer.clone()
            }))
        }
        OperationKind::Load { pointer, access } => one(RuntimeValue::Scalar(execute_scalar_load(
            engine, values, *pointer, *access, &site,
        )?)),
        OperationKind::GuardedLoad {
            pointer,
            predicate,
            fallback,
            access,
        } => {
            let predicate = scalar_value(engine, values, *predicate, &site)?
                .as_bool()
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::RuntimeType {
                            value: Some(*predicate),
                            expected: "boolean guarded-load predicate",
                        },
                    )
                })?;
            let value = if predicate {
                execute_scalar_load(engine, values, *pointer, *access, &site)?
            } else {
                scalar_value(engine, values, *fallback, &site)?
            };
            one(RuntimeValue::Scalar(value))
        }
        OperationKind::Store {
            pointer,
            value,
            access,
        } => {
            let RuntimeValue::Pointer(pointer_value) =
                runtime_value(engine, values, *pointer, &site)?
            else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*pointer),
                        expected: "pointer",
                    },
                ));
            };
            let stored = scalar_value(engine, values, *value, &site)?;
            let bytes = engine
                .memory
                .validate_store(pointer_value, *access, stored, engine.target)
                .map_err(|kind| engine.at(site, kind))?;
            if pointer_value.address_space == AddressSpace::Global {
                engine.record_access(
                    &site,
                    pointer_value.allocation,
                    pointer_value.byte_offset,
                    bytes,
                    true,
                    false,
                )?;
            }
            engine.observe_and_commit_store(&site, pointer_value, stored, bytes)?;
            Ok(SmallResults::None)
        }
        OperationKind::WorkgroupMemory(memory) => one(RuntimeValue::Pointer(
            engine.workgroup_pointer(site, memory)?,
        )),
        OperationKind::Atomic(atomic) => execute_atomic(engine, values, atomic, &site),
        OperationKind::Fence(fence) => {
            if fence.semantics.ordering != MemoryOrdering::Relaxed {
                engine.unmodeled_atomic_or_fence_happens_before = true;
            }
            let address_space_mask = address_space_mask(&fence.semantics.address_spaces);
            engine.event(
                &site,
                SimulationEventKindV1::MemoryFence {
                    memory_scope: fence.memory_scope,
                    ordering: fence.semantics.ordering,
                    address_space_mask,
                },
            )?;
            engine.debug_fence(site, fence);
            Ok(SmallResults::None)
        }
        OperationKind::MemoryIntrinsic(_)
        | OperationKind::Barrier(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::Matrix(_)
        | OperationKind::Wave(_)
        | OperationKind::Gfx950LdsTranspose(_)
        | OperationKind::InlineAssembly(_) => Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "unsupported operation passed preflight",
            ),
        )),
    }
}

#[inline(never)]
fn execute_float_call(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: SoftFloatOperationV1,
    arguments: &[ValueId],
    site: CompactSite,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    if arguments.len() > 3 {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant("bounded float operation arity"),
        ));
    }
    let mut operands = [ScalarBitsV1::boolean(false); 3];
    for (destination, value) in operands.iter_mut().zip(arguments) {
        *destination = scalar_value(engine, values, *value, &site)?;
    }
    Ok(SmallResults::One(RuntimeValue::Scalar(
        crate::soft_float::execute_compact_operation_v1(
            operation,
            &operands[..arguments.len()],
            engine.target,
        )
        .map_err(map_soft_float_error)
        .map_err(|kind| engine.at(site, kind))?,
    )))
}

fn execute_atomic(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    atomic: &Atomic,
    site: &CompactSite,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    let RuntimeValue::Pointer(pointer) = runtime_value(engine, values, atomic.pointer, site)?
    else {
        return Err(engine.at(
            *site,
            SimulationExecutionErrorKindV1::RuntimeType {
                value: Some(atomic.pointer),
                expected: "atomic pointer",
            },
        ));
    };
    let pointer = pointer.clone();
    let operand = atomic
        .value
        .map(|value| scalar_value(engine, values, value, site))
        .transpose()?;
    let compare = atomic
        .compare
        .map(|value| scalar_value(engine, values, value, site))
        .transpose()?;
    let invocation = engine.invocation.ok_or_else(|| {
        engine.at(
            *site,
            SimulationExecutionErrorKindV1::InternalInvariant("atomic invocation"),
        )
    })?;
    if atomic.ordering != MemoryOrdering::Relaxed
        || atomic
            .failure_ordering
            .is_some_and(|ordering| ordering != MemoryOrdering::Relaxed)
    {
        engine.unmodeled_atomic_or_fence_happens_before = true;
    }

    let previous = if atomic.kind == AtomicKind::Store {
        None
    } else {
        Some(
            engine
                .memory
                .load(&pointer, atomic.access, engine.target, invocation)
                .map_err(|kind| engine.at(*site, kind))?,
        )
    };
    let (committed, compare_exchange_success) = match atomic.kind {
        AtomicKind::Load => (None, None),
        AtomicKind::Store => (operand, None),
        AtomicKind::CompareExchange => {
            let old = previous.ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant("atomic compare old value"),
                )
            })?;
            let expected = compare.ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant("atomic compare operand"),
                )
            })?;
            let success = old == expected;
            (success.then_some(operand).flatten(), Some(success))
        }
        kind => {
            let old = previous.ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant("atomic RMW old value"),
                )
            })?;
            let operand = operand.ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant("atomic RMW operand"),
                )
            })?;
            (
                Some(
                    atomic_rmw_value(kind, old, operand, engine.target)
                        .map_err(|kind| engine.at(*site, kind))?,
                ),
                None,
            )
        }
    };

    let width = if let Some(value) = committed {
        engine
            .memory
            .validate_store(&pointer, atomic.access, value, engine.target)
            .map_err(|kind| engine.at(*site, kind))?
    } else {
        engine.target.scalar_bytes(pointer.element).ok_or_else(|| {
            engine.at(
                *site,
                SimulationExecutionErrorKindV1::InternalInvariant(
                    "preflighted atomic scalar width",
                ),
            )
        })?
    };

    if pointer.address_space == AddressSpace::Global {
        engine.record_access(
            site,
            pointer.allocation,
            pointer.byte_offset,
            width,
            committed.is_some(),
            true,
        )?;
    }
    engine.event(
        site,
        SimulationEventKindV1::MemoryAtomic {
            allocation: pointer.allocation,
            offset: pointer.byte_offset,
            bytes: width,
            kind: atomic.kind,
            previous,
            committed,
            compare_exchange_success,
            scope: atomic.scope,
            ordering: atomic.ordering,
            failure_ordering: atomic.failure_ordering,
        },
    )?;

    let debug_access = match (previous, committed) {
        (Some(_), Some(_)) => SimulationDebugMemoryAccessV1::AtomicReadWriteCommitted,
        (Some(_), None) => SimulationDebugMemoryAccessV1::AtomicRead,
        (None, Some(_)) => SimulationDebugMemoryAccessV1::AtomicWriteCommitted,
        (None, None) => {
            return Err(engine.at(
                *site,
                SimulationExecutionErrorKindV1::InternalInvariant("empty atomic effect"),
            ));
        }
    };
    let debug_value = committed.or(previous).ok_or_else(|| {
        engine.at(
            *site,
            SimulationExecutionErrorKindV1::InternalInvariant("atomic debug value"),
        )
    })?;
    if let Some(value) = committed {
        let prepared = match engine.memory.prepare_store(&pointer, width) {
            Ok(prepared) => prepared,
            Err(kind) => {
                return Err(SimulationExecutionErrorV1 {
                    invocation: engine.invocation,
                    site: Some(engine.materialize_site(*site)),
                    kind,
                    observation_failure: None,
                });
            }
        };
        prepared.commit(value);
        engine
            .memory
            .mark_workgroup_atomic(&pointer, width)
            .map_err(|kind| engine.at(*site, kind))?;
    }
    engine.debug_memory(*site, debug_access, &pointer, width, debug_value);

    Ok(match atomic.kind {
        AtomicKind::Store => SmallResults::None,
        AtomicKind::CompareExchange => {
            let old = previous.ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "compare-exchange result old value",
                    ),
                )
            })?;
            let success = compare_exchange_success.ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "compare-exchange result outcome",
                    ),
                )
            })?;
            SmallResults::Two(
                RuntimeValue::Scalar(old),
                RuntimeValue::Scalar(ScalarBitsV1::boolean(success)),
            )
        }
        _ => SmallResults::One(RuntimeValue::Scalar(previous.ok_or_else(|| {
            engine.at(
                *site,
                SimulationExecutionErrorKindV1::InternalInvariant("atomic result old value"),
            )
        })?)),
    })
}

fn atomic_rmw_value(
    kind: AtomicKind,
    old: ScalarBitsV1,
    operand: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SimulationExecutionErrorKindV1> {
    if old.ty() != operand.ty() || !old.ty().is_integer() || old.ty() == ScalarType::Index {
        return Err(SimulationExecutionErrorKindV1::InternalInvariant(
            "preflighted integer atomic operands",
        ));
    }
    let width = scalar_width(old, target)?;
    let bits = match kind {
        AtomicKind::Exchange => operand.bits(),
        AtomicKind::Add => old.bits().wrapping_add(operand.bits()) & mask(width),
        AtomicKind::Subtract => old.bits().wrapping_sub(operand.bits()) & mask(width),
        AtomicKind::Min if old.ty().is_signed_integer() => {
            if signed_value(old, target)? <= signed_value(operand, target)? {
                old.bits()
            } else {
                operand.bits()
            }
        }
        AtomicKind::Max if old.ty().is_signed_integer() => {
            if signed_value(old, target)? >= signed_value(operand, target)? {
                old.bits()
            } else {
                operand.bits()
            }
        }
        AtomicKind::Min => old.bits().min(operand.bits()),
        AtomicKind::Max => old.bits().max(operand.bits()),
        AtomicKind::BitAnd => old.bits() & operand.bits(),
        AtomicKind::BitOr => old.bits() | operand.bits(),
        AtomicKind::BitXor => old.bits() ^ operand.bits(),
        AtomicKind::Load | AtomicKind::Store | AtomicKind::CompareExchange => {
            return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "non-RMW atomic dispatch",
            ));
        }
    };
    ScalarBitsV1::new(old.ty(), bits & mask(width), target)
        .map_err(|_| SimulationExecutionErrorKindV1::InternalInvariant("atomic result bits"))
}

fn address_space_mask(address_spaces: &std::collections::BTreeSet<AddressSpace>) -> u8 {
    address_spaces.iter().fold(0_u8, |mask, address_space| {
        let bit = match address_space {
            AddressSpace::Private => 0,
            AddressSpace::Workgroup => 1,
            AddressSpace::Global => 2,
            AddressSpace::Constant => 3,
            AddressSpace::Generic => 4,
        };
        mask | (1_u8 << bit)
    })
}

fn execute_scalar_load(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    pointer: ValueId,
    access: MemoryAccess,
    site: &CompactSite,
) -> Result<ScalarBitsV1, SimulationExecutionErrorV1> {
    let RuntimeValue::Pointer(pointer_value) = runtime_value(engine, values, pointer, site)? else {
        return Err(engine.at(
            *site,
            SimulationExecutionErrorKindV1::RuntimeType {
                value: Some(pointer),
                expected: "pointer",
            },
        ));
    };
    let value = engine
        .memory
        .load(
            pointer_value,
            access,
            engine.target,
            engine.invocation.ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant("load invocation"),
                )
            })?,
        )
        .map_err(|kind| engine.at(*site, kind))?;
    let bytes = engine
        .target
        .scalar_bytes(pointer_value.element)
        .ok_or_else(|| {
            engine.at(
                *site,
                SimulationExecutionErrorKindV1::InternalInvariant("preflighted load element"),
            )
        })?;
    if pointer_value.address_space == AddressSpace::Global {
        engine.record_access(
            site,
            pointer_value.allocation,
            pointer_value.byte_offset,
            bytes,
            false,
            false,
        )?;
    }
    engine.event(
        site,
        SimulationEventKindV1::MemoryRead {
            allocation: pointer_value.allocation,
            offset: pointer_value.byte_offset,
            bytes,
        },
    )?;
    engine.debug_memory(
        *site,
        SimulationDebugMemoryAccessV1::Read,
        pointer_value,
        bytes,
        value,
    );
    Ok(value)
}

fn operation_site(function: usize, block: &BasicBlock, ordinal: usize) -> CompactSite {
    CompactSite {
        function,
        block: block.id,
        operation: Some(u32::try_from(ordinal).unwrap_or(u32::MAX)),
    }
}

fn terminator_site(function: usize, block: &BasicBlock) -> CompactSite {
    CompactSite {
        function,
        block: block.id,
        operation: None,
    }
}

fn bind_block_arguments(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    function: usize,
    block: &BasicBlock,
    incoming: &[RuntimeValue],
    values: &mut HashMap<ValueId, RuntimeValue>,
) -> Result<(), SimulationExecutionErrorV1> {
    if block.parameters.len() != incoming.len() {
        return Err(
            engine.fail(SimulationExecutionErrorKindV1::BlockArgumentArity {
                expected: block.parameters.len(),
                actual: incoming.len(),
            }),
        );
    }
    for (definition, value) in block.parameters.iter().zip(incoming) {
        let site = CompactSite {
            function,
            block: block.id,
            operation: None,
        };
        bind_typed_value(engine, values, definition, value.clone(), &site)?;
    }
    Ok(())
}

fn bind_typed_value(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    definition: &ValueDef,
    value: RuntimeValue,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    if runtime_type(&value) != definition.ty {
        return Err(engine.at(
            *site,
            SimulationExecutionErrorKindV1::RuntimeType {
                value: Some(definition.id),
                expected: "declared SSA type",
            },
        ));
    }
    bind_runtime_value(engine, values, definition.id, value)
}

fn bind_runtime_value(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &mut HashMap<ValueId, RuntimeValue>,
    id: ValueId,
    value: RuntimeValue,
) -> Result<(), SimulationExecutionErrorV1> {
    if !values.contains_key(&id) && values.len() == engine.limits.max_ssa_values {
        return Err(engine.fail(SimulationExecutionErrorKindV1::SsaValueLimit {
            limit: engine.limits.max_ssa_values,
        }));
    }
    values.insert(id, value);
    Ok(())
}

fn runtime_type(value: &RuntimeValue) -> Type {
    match value {
        RuntimeValue::Scalar(value) => Type::Scalar(value.ty()),
        RuntimeValue::Pointer(pointer) => Type::pointer(
            Type::Scalar(pointer.element),
            pointer.address_space,
            pointer.access,
        ),
        RuntimeValue::Slice(slice) => Type::slice(
            Type::Scalar(slice.element),
            slice.address_space,
            slice.access,
        ),
    }
}

fn runtime_value<'a>(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &'a HashMap<ValueId, RuntimeValue>,
    id: ValueId,
    site: &CompactSite,
) -> Result<&'a RuntimeValue, SimulationExecutionErrorV1> {
    values
        .get(&id)
        .ok_or_else(|| engine.at(*site, SimulationExecutionErrorKindV1::UndefinedValue(id)))
}

fn scalar_value(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    id: ValueId,
    site: &CompactSite,
) -> Result<ScalarBitsV1, SimulationExecutionErrorV1> {
    match runtime_value(engine, values, id, site)? {
        RuntimeValue::Scalar(value) => Ok(*value),
        _ => Err(engine.at(
            *site,
            SimulationExecutionErrorKindV1::RuntimeType {
                value: Some(id),
                expected: "scalar",
            },
        )),
    }
}

fn resolve_values_into(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    ids: &[ValueId],
    site: &CompactSite,
    resolved: &mut Vec<RuntimeValue>,
) -> Result<(), SimulationExecutionErrorV1> {
    resolved.clear();
    if resolved.capacity() < ids.len() {
        resolved
            .try_reserve_exact(ids.len())
            .map_err(|_| engine.at(*site, SimulationExecutionErrorKindV1::AllocationFailure))?;
    }
    for id in ids {
        resolved.push(runtime_value(engine, values, *id, site)?.clone());
    }
    Ok(())
}

fn try_clone_slice<T: Clone>(source: &[T]) -> Result<Vec<T>, SimulationExecutionErrorKindV1> {
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(source.len())
        .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
    cloned.extend_from_slice(source);
    Ok(cloned)
}

fn try_filled<T: Clone>(length: usize, value: T) -> Result<Vec<T>, SimulationExecutionErrorKindV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
    values.resize(length, value);
    Ok(values)
}

fn top_level_error(kind: SimulationExecutionErrorKindV1) -> SimulationExecutionErrorV1 {
    SimulationExecutionErrorV1 {
        invocation: None,
        site: None,
        kind,
        observation_failure: None,
    }
}

fn intrinsic_value(kind: IntrinsicKind, invocation: SimulationInvocationV1) -> u64 {
    let axis = |axis| match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    };
    match kind {
        IntrinsicKind::LaunchExtent { axis: selected } => invocation.launch_extent[axis(selected)],
        IntrinsicKind::InvocationIndex {
            kind,
            axis: selected,
        } => {
            let selected = axis(selected);
            match kind {
                IndexKind::Global => invocation.global[selected],
                IndexKind::Workgroup => invocation.workgroup[selected],
                IndexKind::Local => u64::from(invocation.local[selected]),
                IndexKind::WorkgroupSize => u64::from(invocation.workgroup_size[selected]),
                IndexKind::WorkgroupCount => invocation.workgroup_count[selected],
            }
        }
    }
}

fn constant_scalar(
    constant: &Constant,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SimulationExecutionErrorKindV1> {
    let (ty, bits) = match *constant {
        Constant::Bool(value) => return Ok(ScalarBitsV1::boolean(value)),
        Constant::I8(value) => (ScalarType::I8, value as u8 as u128),
        Constant::I16(value) => (ScalarType::I16, value as u16 as u128),
        Constant::I32(value) => (ScalarType::I32, value as u32 as u128),
        Constant::I64(value) => (ScalarType::I64, value as u64 as u128),
        Constant::U8(value) => (ScalarType::U8, value as u128),
        Constant::U16(value) => (ScalarType::U16, value as u128),
        Constant::U32(value) => (ScalarType::U32, value as u128),
        Constant::U64(value) => (ScalarType::U64, value as u128),
        Constant::Index(value) => (ScalarType::Index, value as u128),
        Constant::F16Bits(value) => (ScalarType::F16, value as u128),
        Constant::Bf16Bits(value) => (ScalarType::Bf16, value as u128),
        Constant::F32Bits(value) => (ScalarType::F32, value as u128),
        Constant::F64Bits(value) => (ScalarType::F64, value as u128),
    };
    ScalarBitsV1::new(ty, bits, target)
        .map_err(|_| SimulationExecutionErrorKindV1::IntegerOutOfRange)
}

fn execute_unary(
    op: UnaryOp,
    value: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SimulationExecutionErrorKindV1> {
    if !supports_unary(op, value.ty()) {
        return Err(SimulationExecutionErrorKindV1::InternalInvariant(
            "unsupported unary passed preflight",
        ));
    }
    if value.ty().is_float() {
        return crate::soft_float::execute_unary_v1(op, value, target)
            .map_err(map_soft_float_error);
    }
    if value.ty() == ScalarType::Bool {
        return match op {
            UnaryOp::Not => Ok(ScalarBitsV1::boolean(value.bits() == 0)),
            UnaryOp::Negate => Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "boolean negate passed preflight",
            )),
        };
    }
    let width = scalar_width(value, target)?;
    let bits = match op {
        UnaryOp::Not => !value.bits() & mask(width),
        UnaryOp::Negate => {
            if !value.ty().is_signed_integer() {
                return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                    "unsigned negate passed preflight",
                ));
            }
            let signed = signed_value(value, target)?;
            let negated = signed.checked_neg().ok_or(
                SimulationExecutionErrorKindV1::UndefinedIntegerOperation(
                    "signed negation overflow",
                ),
            )?;
            if !signed_in_range(negated, width) {
                return Err(SimulationExecutionErrorKindV1::UndefinedIntegerOperation(
                    "signed negation overflow",
                ));
            }
            negated as u128 & mask(width)
        }
    };
    ScalarBitsV1::new(value.ty(), bits, target)
        .map_err(|_| SimulationExecutionErrorKindV1::InternalInvariant("unary bits"))
}

fn execute_binary(
    op: BinaryOp,
    lhs: ScalarBitsV1,
    rhs: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<SmallResults<ScalarBitsV1>, SimulationExecutionErrorKindV1> {
    if !supports_binary(op, lhs.ty(), rhs.ty()) {
        return Err(SimulationExecutionErrorKindV1::InternalInvariant(
            "unsupported binary passed preflight",
        ));
    }
    if lhs.ty().is_float() {
        return Ok(SmallResults::One(
            crate::soft_float::execute_binary_v1(op, lhs, rhs, target)
                .map_err(map_soft_float_error)?,
        ));
    }
    if lhs.ty() == ScalarType::Bool {
        let value = match op {
            BinaryOp::BitAnd => lhs.bits() & rhs.bits(),
            BinaryOp::BitOr => lhs.bits() | rhs.bits(),
            BinaryOp::BitXor => lhs.bits() ^ rhs.bits(),
            _ => {
                return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                    "boolean binary",
                ));
            }
        };
        return Ok(SmallResults::One(ScalarBitsV1::boolean(value != 0)));
    }
    if let BinaryOp::Checked(operator) = op {
        let (bits, overflow) = checked_binary(operator, lhs, rhs, target)?;
        return Ok(SmallResults::Two(
            ScalarBitsV1::new(lhs.ty(), bits, target)
                .map_err(|_| SimulationExecutionErrorKindV1::InternalInvariant("checked bits"))?,
            ScalarBitsV1::boolean(overflow),
        ));
    }
    let width = scalar_width(lhs, target)?;
    let bits = match op {
        BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply => {
            defined_arithmetic(op, lhs, rhs, target)?
        }
        BinaryOp::Divide | BinaryOp::Remainder => defined_division(op, lhs, rhs, target)?,
        BinaryOp::BitAnd => lhs.bits() & rhs.bits(),
        BinaryOp::BitOr => lhs.bits() | rhs.bits(),
        BinaryOp::BitXor => lhs.bits() ^ rhs.bits(),
        BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
            let amount = u32::try_from(rhs.bits()).map_err(|_| {
                SimulationExecutionErrorKindV1::UndefinedIntegerOperation("shift amount")
            })?;
            if amount >= u32::from(width) {
                return Err(SimulationExecutionErrorKindV1::UndefinedIntegerOperation(
                    "shift amount is not less than integer width",
                ));
            }
            match op {
                BinaryOp::ShiftLeft => (lhs.bits() << amount) & mask(width),
                BinaryOp::ShiftRight if lhs.ty().is_signed_integer() => {
                    (signed_value(lhs, target)? >> amount) as u128 & mask(width)
                }
                BinaryOp::ShiftRight => lhs.bits() >> amount,
                _ => {
                    return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                        "shift operation",
                    ));
                }
            }
        }
        BinaryOp::Checked(_) => {
            return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "checked binary dispatch",
            ));
        }
    };
    Ok(SmallResults::One(
        ScalarBitsV1::new(lhs.ty(), bits & mask(width), target)
            .map_err(|_| SimulationExecutionErrorKindV1::InternalInvariant("binary bits"))?,
    ))
}

fn checked_binary(
    op: CheckedBinaryOperator,
    lhs: ScalarBitsV1,
    rhs: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<(u128, bool), SimulationExecutionErrorKindV1> {
    let width = scalar_width(lhs, target)?;
    let mask = mask(width);
    let wrapped = match op {
        CheckedBinaryOperator::Add => lhs.bits().wrapping_add(rhs.bits()) & mask,
        CheckedBinaryOperator::Subtract => lhs.bits().wrapping_sub(rhs.bits()) & mask,
        CheckedBinaryOperator::Multiply => lhs.bits().wrapping_mul(rhs.bits()) & mask,
    };
    let overflow = if lhs.ty().is_signed_integer() {
        let left = signed_value(lhs, target)?;
        let right = signed_value(rhs, target)?;
        match op {
            CheckedBinaryOperator::Add => left.checked_add(right),
            CheckedBinaryOperator::Subtract => left.checked_sub(right),
            CheckedBinaryOperator::Multiply => left.checked_mul(right),
        }
        .is_none_or(|value| !signed_in_range(value, width))
    } else {
        match op {
            CheckedBinaryOperator::Add => lhs.bits().checked_add(rhs.bits()),
            CheckedBinaryOperator::Subtract => lhs.bits().checked_sub(rhs.bits()),
            CheckedBinaryOperator::Multiply => lhs.bits().checked_mul(rhs.bits()),
        }
        .is_none_or(|value| value > mask)
    };
    Ok((wrapped, overflow))
}

fn defined_arithmetic(
    op: BinaryOp,
    lhs: ScalarBitsV1,
    rhs: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<u128, SimulationExecutionErrorKindV1> {
    let width = scalar_width(lhs, target)?;
    if lhs.ty().is_signed_integer() {
        let left = signed_value(lhs, target)?;
        let right = signed_value(rhs, target)?;
        let result = match op {
            BinaryOp::Add => left.checked_add(right),
            BinaryOp::Subtract => left.checked_sub(right),
            BinaryOp::Multiply => left.checked_mul(right),
            _ => {
                return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                    "arithmetic operation",
                ));
            }
        }
        .filter(|value| signed_in_range(*value, width))
        .ok_or(SimulationExecutionErrorKindV1::UndefinedIntegerOperation(
            "core signed arithmetic overflow",
        ))?;
        Ok(result as u128 & mask(width))
    } else {
        let result = match op {
            BinaryOp::Add => lhs.bits().checked_add(rhs.bits()),
            BinaryOp::Subtract => lhs.bits().checked_sub(rhs.bits()),
            BinaryOp::Multiply => lhs.bits().checked_mul(rhs.bits()),
            _ => {
                return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                    "arithmetic operation",
                ));
            }
        }
        .filter(|value| *value <= mask(width))
        .ok_or(SimulationExecutionErrorKindV1::UndefinedIntegerOperation(
            "core unsigned arithmetic overflow",
        ))?;
        Ok(result)
    }
}

fn defined_division(
    op: BinaryOp,
    lhs: ScalarBitsV1,
    rhs: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<u128, SimulationExecutionErrorKindV1> {
    let width = scalar_width(lhs, target)?;
    if rhs.bits() == 0 {
        return Err(SimulationExecutionErrorKindV1::UndefinedIntegerOperation(
            "division by zero",
        ));
    }
    if lhs.ty().is_signed_integer() {
        let left = signed_value(lhs, target)?;
        let right = signed_value(rhs, target)?;
        let result = match op {
            BinaryOp::Divide => left.checked_div(right),
            BinaryOp::Remainder => left.checked_rem(right),
            _ => {
                return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                    "division operation",
                ));
            }
        }
        .ok_or(SimulationExecutionErrorKindV1::UndefinedIntegerOperation(
            "signed division overflow",
        ))?;
        Ok(result as u128 & mask(width))
    } else {
        Ok(match op {
            BinaryOp::Divide => lhs.bits() / rhs.bits(),
            BinaryOp::Remainder => lhs.bits() % rhs.bits(),
            _ => {
                return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                    "division operation",
                ));
            }
        })
    }
}

fn execute_compare(
    predicate: ComparePredicate,
    lhs: ScalarBitsV1,
    rhs: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<bool, SimulationExecutionErrorKindV1> {
    if !supports_compare(predicate, lhs.ty(), rhs.ty()) {
        return Err(SimulationExecutionErrorKindV1::InternalInvariant(
            "unsupported compare passed preflight",
        ));
    }
    if lhs.ty() == ScalarType::Bool {
        return match predicate {
            ComparePredicate::Equal => Ok(lhs.bits() == rhs.bits()),
            ComparePredicate::NotEqual => Ok(lhs.bits() != rhs.bits()),
            _ => Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "ordered boolean compare passed preflight",
            )),
        };
    }
    if lhs.ty().is_signed_integer() {
        compare_ordered(
            predicate,
            signed_value(lhs, target)?,
            signed_value(rhs, target)?,
        )
    } else if lhs.ty().is_integer() {
        compare_ordered(predicate, lhs.bits(), rhs.bits())
    } else if lhs.ty().is_float() {
        crate::soft_float::execute_compare_v1(predicate, lhs, rhs).map_err(map_soft_float_error)
    } else {
        Err(SimulationExecutionErrorKindV1::InternalInvariant(
            "unsupported compare type passed preflight",
        ))
    }
}

fn compare_ordered<T: Ord>(
    predicate: ComparePredicate,
    lhs: T,
    rhs: T,
) -> Result<bool, SimulationExecutionErrorKindV1> {
    Ok(match predicate {
        ComparePredicate::Equal => lhs == rhs,
        ComparePredicate::NotEqual => lhs != rhs,
        ComparePredicate::LessThan => lhs < rhs,
        ComparePredicate::LessThanOrEqual => lhs <= rhs,
        ComparePredicate::GreaterThan => lhs > rhs,
        ComparePredicate::GreaterThanOrEqual => lhs >= rhs,
    })
}

fn execute_cast(
    kind: CastKind,
    value: ScalarBitsV1,
    to: ScalarType,
    target: SimulationTargetV1,
) -> Result<ScalarBitsV1, SimulationExecutionErrorKindV1> {
    if !supported_cast(kind, value.ty(), to, target) {
        return Err(SimulationExecutionErrorKindV1::InternalInvariant(
            "unsupported cast passed preflight",
        ));
    }
    if matches!(
        kind,
        CastKind::FloatExtend
            | CastKind::FloatTruncate
            | CastKind::IntegerToFloat
            | CastKind::FloatToInteger
    ) {
        return crate::soft_float::execute_cast_v1(kind, value, to, target)
            .map_err(map_soft_float_error);
    }
    let from_width = scalar_width(value, target)?;
    let to_width =
        target
            .scalar_bits(to)
            .ok_or(SimulationExecutionErrorKindV1::InternalInvariant(
                "preflighted cast target",
            ))?;
    let bits = match kind {
        CastKind::Truncate => value.bits() & mask(to_width),
        CastKind::ZeroExtend | CastKind::Bitcast => value.bits(),
        CastKind::SignExtend => signed_value(value, target)? as u128 & mask(to_width),
        CastKind::FloatExtend
        | CastKind::FloatTruncate
        | CastKind::IntegerToFloat
        | CastKind::FloatToInteger => unreachable!("handled software-float cast"),
    };
    let structurally_valid = match kind {
        CastKind::Truncate => to_width < from_width,
        CastKind::ZeroExtend | CastKind::SignExtend => to_width > from_width,
        CastKind::Bitcast => to_width == from_width,
        _ => false,
    };
    if !structurally_valid {
        return Err(SimulationExecutionErrorKindV1::InternalInvariant(
            "invalid cast passed preflight",
        ));
    }
    ScalarBitsV1::new(to, bits, target)
        .map_err(|_| SimulationExecutionErrorKindV1::IntegerOutOfRange)
}

const fn map_soft_float_error(error: SoftFloatErrorV1) -> SimulationExecutionErrorKindV1 {
    match error {
        SoftFloatErrorV1::InvalidIntegerConversion => {
            SimulationExecutionErrorKindV1::IntegerOutOfRange
        }
        SoftFloatErrorV1::InternalInvariant(message) => {
            SimulationExecutionErrorKindV1::InternalInvariant(message)
        }
    }
}

fn scalar_width(
    value: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<u16, SimulationExecutionErrorKindV1> {
    target
        .scalar_bits(value.ty())
        .ok_or(SimulationExecutionErrorKindV1::InternalInvariant(
            "unsupported scalar passed preflight",
        ))
}

fn signed_value(
    value: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<i128, SimulationExecutionErrorKindV1> {
    let width = scalar_width(value, target)?;
    if !value.ty().is_signed_integer() {
        return Err(SimulationExecutionErrorKindV1::RuntimeType {
            value: None,
            expected: "signed integer",
        });
    }
    if width == 128 {
        return Ok(value.bits() as i128);
    }
    let sign = 1_u128 << (width - 1);
    let extended = if value.bits() & sign == 0 {
        value.bits()
    } else {
        value.bits() | !mask(width)
    };
    Ok(extended as i128)
}

fn signed_in_range(value: i128, width: u16) -> bool {
    if width == 128 {
        true
    } else {
        let limit = 1_i128 << (width - 1);
        (-limit..limit).contains(&value)
    }
}

fn scalar_nonnegative_usize(
    value: ScalarBitsV1,
    target: SimulationTargetV1,
) -> Result<usize, SimulationExecutionErrorKindV1> {
    if value.ty().is_signed_integer() && signed_value(value, target)? < 0 {
        return Err(SimulationExecutionErrorKindV1::IntegerOutOfRange);
    }
    usize::try_from(value.bits()).map_err(|_| SimulationExecutionErrorKindV1::IntegerOutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boolean_zero_extension_preserves_bits_across_index_widths() {
        for target in [
            SimulationTargetV1::little_endian(crate::IndexWidthV1::Bits32),
            SimulationTargetV1::little_endian(crate::IndexWidthV1::Bits64),
        ] {
            for value in [false, true] {
                let fixed = execute_cast(
                    CastKind::ZeroExtend,
                    ScalarBitsV1::boolean(value),
                    ScalarType::I64,
                    target,
                )
                .unwrap();
                assert_eq!(fixed.ty(), ScalarType::I64);
                assert_eq!(fixed.bits(), value as u128);

                let index = execute_cast(
                    CastKind::ZeroExtend,
                    ScalarBitsV1::boolean(value),
                    ScalarType::Index,
                    target,
                )
                .unwrap();
                assert_eq!(index.ty(), ScalarType::Index);
                assert_eq!(index.bits(), value as u128);
                assert!(index.matches_target(target));
            }
        }
    }

    #[test]
    fn admitted_binary_results_use_fixed_inline_storage() {
        let target = SimulationTargetV1::amdgpu_64();
        let checked = execute_binary(
            BinaryOp::Checked(CheckedBinaryOperator::Add),
            ScalarBitsV1::u32(u32::MAX),
            ScalarBitsV1::u32(1),
            target,
        )
        .expect("checked add");
        assert!(matches!(checked, SmallResults::Two(_, _)));

        let ordinary = execute_binary(
            BinaryOp::Add,
            ScalarBitsV1::u32(1),
            ScalarBitsV1::u32(2),
            target,
        )
        .expect("ordinary add");
        assert!(matches!(ordinary, SmallResults::One(_)));
        assert!(
            std::mem::size_of::<SmallResults<RuntimeValue>>()
                <= 2 * std::mem::size_of::<RuntimeValue>() + std::mem::align_of::<RuntimeValue>()
        );
    }
}

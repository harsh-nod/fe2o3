use std::mem::size_of;

use fe2o3_kernel_ir::{AccessMode, ScalarType};
use sha2::{Digest, Sha256};

use crate::resident::reserved_vec_bytes;
use crate::{
    BufferArgumentV1, DynamicWorkgroupMemoryRequestV1, EventPolicyV1, IndexWidthV1,
    SimulationArgumentV1, SimulationInvocationV1, SimulationKernelIrIdentityV1, SimulationLimitsV1,
    SimulationPlanV1, SimulationRequestV1, SimulationTargetV1,
};

mod persisted;

pub use persisted::{
    MAX_PERSISTED_SCHEDULE_BYTES_V1, PersistedSimulationScheduleArtifactV1,
    PersistedSimulationScheduleBindingV1, PersistedSimulationScheduleCodecErrorV1,
    PersistedSimulationScheduleDocumentV1,
};

/// Hard upper bound on retained runnable-invocation decisions in one schedule record.
pub const MAX_SCHEDULE_DECISIONS_V1: usize = 4 * 1024 * 1024;

const CONTEXT_DOMAIN_V1: &[u8] = b"FE2O3/KIR-SIM/SCHEDULE-CONTEXT/V1\0";
const CONTEXT_DOMAIN_V2: &[u8] = b"FE2O3/KIR-SIM/SCHEDULE-CONTEXT/V2\0";
const CONTEXT_DOMAIN_V3: &[u8] = b"FE2O3/KIR-SIM/SCHEDULE-CONTEXT/V3\0";
const CONTEXT_DOMAIN_V4: &[u8] = b"FE2O3/KIR-SIM/SCHEDULE-CONTEXT/V4\0";
const DYNAMIC_CONTEXT_DOMAIN_V1: &[u8] = b"FE2O3/KIR-SIM/DYNAMIC-LDS-SCHEDULE-CONTEXT/V1\0";
const TRANSCRIPT_DOMAIN_V1: &[u8] = b"FE2O3/KIR-SIM/SCHEDULE-TRANSCRIPT/V1\0";
const RECORD_INTEGRITY_DOMAIN_V1: &[u8] = b"FE2O3/KIR-SIM/SCHEDULE-RECORD/V1\0";

/// Semantic CPU ordering used by one simulation.
///
/// These identities describe simulator ordering only. They make no claim about
/// GPU scheduling, timing, performance, or physical wave execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationScheduleIdentityV1 {
    /// Legacy profile retained for old serialized observations.
    WorkgroupMajorLocalZyxSerialV1,
    /// Workgroup Z/Y/X and local Z/Y/X, cooperatively yielding at barriers.
    WorkgroupMajorLocalZyxCooperativeV1,
    /// Workgroup Z/Y/X with a deterministic seeded permutation of runnable
    /// local invocations at each cooperative phase.
    WorkgroupMajorSeededRunnableCooperativeV1,
}

/// One stable runnable-invocation selection in a cooperative workgroup phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationScheduleDecisionV1 {
    workgroup: [u64; 3],
    phase: u64,
    local: [u32; 3],
}

impl SimulationScheduleDecisionV1 {
    pub(crate) const fn new(workgroup: [u64; 3], phase: u64, local: [u32; 3]) -> Self {
        Self {
            workgroup,
            phase,
            local,
        }
    }

    pub const fn workgroup(self) -> [u64; 3] {
        self.workgroup
    }

    pub const fn phase(self) -> u64 {
        self.phase
    }

    pub const fn local(self) -> [u32; 3] {
        self.local
    }
}

/// Exact successful coverage of one semantic CPU schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationScheduleCoverageV1 {
    decisions: u64,
    workgroups: u64,
    barrier_releases: u64,
}

impl SimulationScheduleCoverageV1 {
    pub const fn decisions(self) -> u64 {
        self.decisions
    }

    pub const fn workgroups(self) -> u64 {
        self.workgroups
    }

    pub const fn barrier_releases(self) -> u64 {
        self.barrier_releases
    }

    /// Coverage is only returned after every workgroup and invocation completed.
    pub const fn is_complete(self) -> bool {
        true
    }
}

/// Opaque, integrity-bound schedule record produced only by a successful run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationScheduleRecordV1 {
    context_identity: [u8; 32],
    transcript_identity: [u8; 32],
    record_integrity: [u8; 32],
    schedule: SimulationScheduleIdentityV1,
    seed: Option<u64>,
    decisions: Vec<SimulationScheduleDecisionV1>,
    coverage: SimulationScheduleCoverageV1,
}

impl SimulationScheduleRecordV1 {
    pub const fn context_identity(&self) -> &[u8; 32] {
        &self.context_identity
    }

    pub const fn transcript_identity(&self) -> &[u8; 32] {
        &self.transcript_identity
    }

    /// Identity of this transcript plus every exact replay decision.
    pub const fn record_integrity(&self) -> &[u8; 32] {
        &self.record_integrity
    }

    pub const fn schedule(&self) -> SimulationScheduleIdentityV1 {
        self.schedule
    }

    pub const fn seed(&self) -> Option<u64> {
        self.seed
    }

    pub fn decisions(&self) -> &[SimulationScheduleDecisionV1] {
        &self.decisions
    }

    pub(crate) fn retained_heap_bytes(&self) -> Option<usize> {
        self.decisions
            .capacity()
            .checked_mul(size_of::<SimulationScheduleDecisionV1>())
    }

    pub const fn coverage(&self) -> SimulationScheduleCoverageV1 {
        self.coverage
    }
}

/// Opt-in schedule operation for one simulation.
#[derive(Clone, Copy, Debug)]
pub enum SimulationScheduleRequestV1<'a> {
    /// Record the canonical cooperative ordering.
    RecordCanonical { max_decisions: usize },
    /// Record a deterministic seeded permutation of runnable invocations.
    RecordSeeded { seed: u64, max_decisions: usize },
    /// Replay one exact completed record.
    Replay(&'a SimulationScheduleRecordV1),
}

/// Fail-closed replay rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationScheduleReplayErrorV1 {
    ContextMismatch,
    TranscriptIntegrity,
    InvalidRecordCoverage,
    MissingDecision,
    UnexpectedWorkgroup,
    UnexpectedPhase,
    LocalNotRunnable,
    DuplicateLocal,
    TrailingDecision,
    CoverageMismatch,
}

#[derive(Clone, Copy)]
enum ScheduledModeV1<'a> {
    Record,
    Replay(&'a SimulationScheduleRecordV1),
    ReduceCanonical,
    ReduceSeeded,
    ReducePrefix(&'a [SimulationScheduleDecisionV1]),
}

pub(crate) enum ExecutionScheduleRequestV1<'a> {
    Public(SimulationScheduleRequestV1<'a>),
    Reduction {
        source: ReductionScheduleSourceV1<'a>,
        max_decisions: usize,
        decisions: &'a mut Vec<SimulationScheduleDecisionV1>,
    },
}

pub(crate) enum ReductionScheduleSourceV1<'a> {
    Canonical,
    Seeded(u64),
    PrefixThenCanonical(&'a [SimulationScheduleDecisionV1]),
}

pub(crate) struct PreparedScheduleV1<'a> {
    current_decision: u64,
    workgroups: u64,
    barrier_releases: u64,
    scheduled: Vec<ScheduledStateV1<'a>>,
    reduction_decisions: Option<&'a mut Vec<SimulationScheduleDecisionV1>>,
}

struct ScheduledStateV1<'a> {
    mode: ScheduledModeV1<'a>,
    context_identity: [u8; 32],
    schedule: SimulationScheduleIdentityV1,
    seed: Option<u64>,
    random_state: u64,
    decisions: Vec<SimulationScheduleDecisionV1>,
    cursor: usize,
    max_decisions: usize,
    order: Vec<usize>,
    records: Vec<SimulationScheduleRecordV1>,
}

pub(crate) enum SchedulePrepareErrorV1 {
    DecisionLimit { actual: usize, limit: usize },
    ResidentLimit { actual: usize, limit: usize },
    AllocationFailure,
    Replay(SimulationScheduleReplayErrorV1),
}

impl<'a> PreparedScheduleV1<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        request: Option<ExecutionScheduleRequestV1<'a>>,
        identity: SimulationKernelIrIdentityV1,
        simulation: &SimulationRequestV1,
        dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        plan: &SimulationPlanV1,
        participants: usize,
        resident_offset: usize,
    ) -> Result<Self, SchedulePrepareErrorV1> {
        let Some(request) = request else {
            return Ok(Self {
                current_decision: 0,
                workgroups: 0,
                barrier_releases: 0,
                scheduled: Vec::new(),
                reduction_decisions: None,
            });
        };
        let context_identity =
            schedule_context_identity_configured(identity, simulation, dynamic, target, limits);
        let (
            mode,
            schedule,
            seed,
            max_decisions,
            retained_decisions,
            resident_decisions,
            needs_order,
        ) = match &request {
            ExecutionScheduleRequestV1::Public(SimulationScheduleRequestV1::Replay(record)) => {
                validate_record(record, context_identity, plan, limits)?;
                (
                    ScheduledModeV1::Replay(record),
                    record.schedule,
                    record.seed,
                    record.decisions.len(),
                    0,
                    record.decisions.capacity(),
                    true,
                )
            }
            ExecutionScheduleRequestV1::Public(SimulationScheduleRequestV1::RecordCanonical {
                max_decisions,
            }) => (
                ScheduledModeV1::Record,
                SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
                None,
                *max_decisions,
                *max_decisions,
                *max_decisions,
                false,
            ),
            ExecutionScheduleRequestV1::Public(SimulationScheduleRequestV1::RecordSeeded {
                seed,
                max_decisions,
            }) => (
                ScheduledModeV1::Record,
                SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1,
                Some(*seed),
                *max_decisions,
                *max_decisions,
                *max_decisions,
                true,
            ),
            ExecutionScheduleRequestV1::Reduction {
                source: ReductionScheduleSourceV1::Canonical,
                max_decisions,
                ..
            } => (
                ScheduledModeV1::ReduceCanonical,
                SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
                None,
                *max_decisions,
                0,
                0,
                false,
            ),
            ExecutionScheduleRequestV1::Reduction {
                source: ReductionScheduleSourceV1::Seeded(seed),
                max_decisions,
                ..
            } => (
                ScheduledModeV1::ReduceSeeded,
                SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1,
                Some(*seed),
                *max_decisions,
                0,
                0,
                true,
            ),
            ExecutionScheduleRequestV1::Reduction {
                source: ReductionScheduleSourceV1::PrefixThenCanonical(prefix),
                max_decisions,
                ..
            } => (
                ScheduledModeV1::ReducePrefix(prefix),
                SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
                None,
                *max_decisions,
                0,
                0,
                true,
            ),
        };
        validate_decision_limit(max_decisions, limits)?;

        let decision_bytes = reserved_vec_bytes::<SimulationScheduleDecisionV1>(resident_decisions)
            .ok_or(SchedulePrepareErrorV1::ResidentLimit {
                actual: usize::MAX,
                limit: limits.max_resident_bytes,
            })?;
        let order_bytes = if needs_order {
            reserved_vec_bytes::<usize>(participants).ok_or(
                SchedulePrepareErrorV1::ResidentLimit {
                    actual: usize::MAX,
                    limit: limits.max_resident_bytes,
                },
            )?
        } else {
            0
        };
        let extra = size_of::<Self>()
            .checked_add(reserved_vec_bytes::<ScheduledStateV1<'_>>(1).ok_or(
                SchedulePrepareErrorV1::ResidentLimit {
                    actual: usize::MAX,
                    limit: limits.max_resident_bytes,
                },
            )?)
            .and_then(|bytes| bytes.checked_add(decision_bytes))
            .and_then(|bytes| bytes.checked_add(order_bytes))
            .and_then(|bytes| {
                if matches!(mode, ScheduledModeV1::Record) {
                    reserved_vec_bytes::<SimulationScheduleRecordV1>(1)
                        .and_then(|record_bytes| bytes.checked_add(record_bytes))
                } else {
                    Some(bytes)
                }
            })
            .ok_or(SchedulePrepareErrorV1::ResidentLimit {
                actual: usize::MAX,
                limit: limits.max_resident_bytes,
            })?;
        let resident = plan
            .resident_bytes()
            .checked_add(extra)
            .and_then(|bytes| bytes.checked_add(resident_offset))
            .ok_or(SchedulePrepareErrorV1::ResidentLimit {
                actual: usize::MAX,
                limit: limits.max_resident_bytes,
            })?;
        if resident > limits.max_resident_bytes {
            return Err(SchedulePrepareErrorV1::ResidentLimit {
                actual: resident,
                limit: limits.max_resident_bytes,
            });
        }

        let mut decisions = Vec::new();
        decisions
            .try_reserve_exact(retained_decisions)
            .map_err(|_| SchedulePrepareErrorV1::AllocationFailure)?;
        let mut order = Vec::new();
        if needs_order {
            order
                .try_reserve_exact(participants)
                .map_err(|_| SchedulePrepareErrorV1::AllocationFailure)?;
        }
        let mut records = Vec::new();
        if matches!(mode, ScheduledModeV1::Record) {
            records
                .try_reserve_exact(1)
                .map_err(|_| SchedulePrepareErrorV1::AllocationFailure)?;
        }
        let mut scheduled = Vec::new();
        scheduled
            .try_reserve_exact(1)
            .map_err(|_| SchedulePrepareErrorV1::AllocationFailure)?;
        scheduled.push(ScheduledStateV1 {
            mode,
            context_identity,
            schedule,
            seed,
            random_state: seed.unwrap_or(0),
            decisions,
            cursor: 0,
            max_decisions,
            order,
            records,
        });
        let reduction_decisions = match request {
            ExecutionScheduleRequestV1::Reduction { decisions, .. } => {
                debug_assert!(decisions.is_empty());
                debug_assert!(decisions.capacity() >= max_decisions);
                Some(decisions)
            }
            ExecutionScheduleRequestV1::Public(_) => None,
        };
        Ok(Self {
            current_decision: 0,
            workgroups: 0,
            barrier_releases: 0,
            scheduled,
            reduction_decisions,
        })
    }

    pub(crate) fn identity(&self) -> SimulationScheduleIdentityV1 {
        self.scheduled.first().map_or(
            SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
            |state| state.schedule,
        )
    }

    pub(crate) const fn current_decision(&self) -> u64 {
        self.current_decision
    }

    pub(crate) fn begin_workgroup(&mut self) {
        self.workgroups += 1;
    }

    pub(crate) fn barrier_released(&mut self) {
        self.barrier_releases += 1;
    }

    pub(crate) fn uses_canonical_order(&self) -> bool {
        self.scheduled.first().is_none_or(|state| {
            matches!(
                state.mode,
                ScheduledModeV1::Record | ScheduledModeV1::ReduceCanonical
            ) && state.seed.is_none()
        })
    }

    pub(crate) fn take_order(
        &mut self,
        machine_count: usize,
        invocation_at: impl Fn(usize) -> SimulationInvocationV1,
        runnable_at: impl Fn(usize) -> bool,
        workgroup: [u64; 3],
        phase: u64,
    ) -> Result<Vec<usize>, SimulationScheduleReplayErrorV1> {
        let state = self
            .scheduled
            .first_mut()
            .expect("non-canonical schedule has admitted state");
        state.order.clear();
        match state.mode {
            ScheduledModeV1::Replay(record) => {
                let runnable = (0..machine_count)
                    .filter(|index| runnable_at(*index))
                    .count();
                for _ in 0..runnable {
                    let decision = *record
                        .decisions
                        .get(state.cursor)
                        .ok_or(SimulationScheduleReplayErrorV1::MissingDecision)?;
                    if decision.workgroup != workgroup {
                        return Err(SimulationScheduleReplayErrorV1::UnexpectedWorkgroup);
                    }
                    if decision.phase != phase {
                        return Err(SimulationScheduleReplayErrorV1::UnexpectedPhase);
                    }
                    let Some(index) = (0..machine_count)
                        .find(|index| invocation_at(*index).local == decision.local)
                    else {
                        return Err(SimulationScheduleReplayErrorV1::LocalNotRunnable);
                    };
                    if !runnable_at(index) {
                        return Err(SimulationScheduleReplayErrorV1::LocalNotRunnable);
                    }
                    if state.order.contains(&index) {
                        return Err(SimulationScheduleReplayErrorV1::DuplicateLocal);
                    }
                    state.order.push(index);
                    state.cursor += 1;
                }
            }
            ScheduledModeV1::Record if state.seed.is_some() => {
                state
                    .order
                    .extend((0..machine_count).filter(|index| runnable_at(*index)));
                for index in (1..state.order.len()).rev() {
                    let swap = next_random_index(&mut state.random_state, index + 1);
                    state.order.swap(index, swap);
                }
            }
            ScheduledModeV1::ReduceSeeded => {
                state
                    .order
                    .extend((0..machine_count).filter(|index| runnable_at(*index)));
                for index in (1..state.order.len()).rev() {
                    let swap = next_random_index(&mut state.random_state, index + 1);
                    state.order.swap(index, swap);
                }
            }
            ScheduledModeV1::ReducePrefix(prefix) => {
                let runnable = (0..machine_count)
                    .filter(|index| runnable_at(*index))
                    .count();
                while state.order.len() < runnable && state.cursor < prefix.len() {
                    let decision = prefix[state.cursor];
                    if decision.workgroup != workgroup {
                        return Err(SimulationScheduleReplayErrorV1::UnexpectedWorkgroup);
                    }
                    if decision.phase != phase {
                        return Err(SimulationScheduleReplayErrorV1::UnexpectedPhase);
                    }
                    let Some(index) = (0..machine_count)
                        .find(|index| invocation_at(*index).local == decision.local)
                    else {
                        return Err(SimulationScheduleReplayErrorV1::LocalNotRunnable);
                    };
                    if !runnable_at(index) {
                        return Err(SimulationScheduleReplayErrorV1::LocalNotRunnable);
                    }
                    if state.order.contains(&index) {
                        return Err(SimulationScheduleReplayErrorV1::DuplicateLocal);
                    }
                    state.order.push(index);
                    state.cursor += 1;
                }
                for index in 0..machine_count {
                    if runnable_at(index) && !state.order.contains(&index) {
                        state.order.push(index);
                    }
                }
            }
            _ => unreachable!("canonical order does not allocate a phase order"),
        }
        Ok(std::mem::take(&mut state.order))
    }

    pub(crate) fn restore_order(&mut self, mut order: Vec<usize>) {
        order.clear();
        self.scheduled
            .first_mut()
            .expect("non-canonical schedule has admitted state")
            .order = order;
    }

    pub(crate) fn selected(
        &mut self,
        invocation: SimulationInvocationV1,
        phase: u64,
    ) -> Result<(), SchedulePrepareErrorV1> {
        let decision = SimulationScheduleDecisionV1 {
            workgroup: invocation.workgroup,
            phase,
            local: invocation.local,
        };
        if let Some(state) = self.scheduled.first_mut() {
            if self.current_decision as usize >= state.max_decisions {
                return Err(SchedulePrepareErrorV1::DecisionLimit {
                    actual: self.current_decision as usize + 1,
                    limit: state.max_decisions,
                });
            }
            if matches!(state.mode, ScheduledModeV1::Record) {
                state.decisions.push(decision);
            }
        }
        if let Some(decisions) = self.reduction_decisions.as_mut() {
            if decisions.len() == decisions.capacity() {
                return Err(SchedulePrepareErrorV1::AllocationFailure);
            }
            decisions.push(decision);
        }
        self.current_decision =
            self.current_decision
                .checked_add(1)
                .ok_or(SchedulePrepareErrorV1::DecisionLimit {
                    actual: usize::MAX,
                    limit: self
                        .scheduled
                        .first()
                        .map_or(usize::MAX, |state| state.max_decisions),
                })?;
        Ok(())
    }

    pub(crate) fn finish(
        self,
        expected_workgroups: u64,
        identity: SimulationKernelIrIdentityV1,
        simulation: &SimulationRequestV1,
        dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
    ) -> Result<PreparedScheduleResultV1, SimulationScheduleReplayErrorV1> {
        let coverage = SimulationScheduleCoverageV1 {
            decisions: self.current_decision,
            workgroups: self.workgroups,
            barrier_releases: self.barrier_releases,
        };
        if self.workgroups != expected_workgroups {
            return Err(SimulationScheduleReplayErrorV1::CoverageMismatch);
        }
        let mut scheduled = self.scheduled;
        let state = scheduled.pop();
        if let Some(ScheduledStateV1 {
            mode: ScheduledModeV1::Replay(record),
            cursor,
            ..
        }) = &state
        {
            if *cursor != record.decisions.len() {
                return Err(SimulationScheduleReplayErrorV1::TrailingDecision);
            }
            if coverage != record.coverage {
                return Err(SimulationScheduleReplayErrorV1::CoverageMismatch);
            }
        }
        if let Some(ScheduledStateV1 {
            mode: ScheduledModeV1::ReducePrefix(prefix),
            cursor,
            ..
        }) = &state
            && *cursor != prefix.len()
        {
            return Err(SimulationScheduleReplayErrorV1::TrailingDecision);
        }
        let schedule = state.as_ref().map_or(
            SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
            |state| state.schedule,
        );
        let seed = state.as_ref().and_then(|state| state.seed);
        let context_identity = state.as_ref().map_or_else(
            || schedule_context_identity_configured(identity, simulation, dynamic, target, limits),
            |state| state.context_identity,
        );
        let transcript_identity = transcript_identity(context_identity, schedule, seed, coverage);
        if let Some(ScheduledStateV1 {
            mode: ScheduledModeV1::Replay(record),
            ..
        }) = &state
            && transcript_identity != record.transcript_identity
        {
            return Err(SimulationScheduleReplayErrorV1::TranscriptIntegrity);
        }
        let mut records = Vec::new();
        if let Some(ScheduledStateV1 {
            mode: ScheduledModeV1::Record,
            decisions,
            records: retained_records,
            ..
        }) = state
        {
            records = retained_records;
            // Recording reserves the caller's decision bound so execution never
            // reallocates. A completed witness retains only realized decisions;
            // this prevents a one-decision run from escaping with a multi-million
            // element capacity.
            let decisions = decisions.into_boxed_slice().into_vec();
            records.push(SimulationScheduleRecordV1 {
                context_identity,
                transcript_identity,
                record_integrity: record_integrity(transcript_identity, &decisions),
                schedule,
                seed,
                decisions,
                coverage,
            });
        }
        Ok(PreparedScheduleResultV1 {
            identity: schedule,
            transcript_identity,
            coverage,
            records,
        })
    }
}

fn next_random_index(random_state: &mut u64, upper: usize) -> usize {
    *random_state = random_state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut value = *random_state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    (value % upper as u64) as usize
}

pub(crate) struct PreparedScheduleResultV1 {
    pub identity: SimulationScheduleIdentityV1,
    pub transcript_identity: [u8; 32],
    pub coverage: SimulationScheduleCoverageV1,
    pub records: Vec<SimulationScheduleRecordV1>,
}

fn validate_decision_limit(
    max_decisions: usize,
    limits: SimulationLimitsV1,
) -> Result<(), SchedulePrepareErrorV1> {
    let limit =
        MAX_SCHEDULE_DECISIONS_V1.min(usize::try_from(limits.max_steps).unwrap_or(usize::MAX));
    if max_decisions == 0 || max_decisions > limit {
        return Err(SchedulePrepareErrorV1::DecisionLimit {
            actual: max_decisions,
            limit,
        });
    }
    Ok(())
}

fn validate_record(
    record: &SimulationScheduleRecordV1,
    context_identity: [u8; 32],
    plan: &SimulationPlanV1,
    limits: SimulationLimitsV1,
) -> Result<(), SchedulePrepareErrorV1> {
    if record.context_identity != context_identity {
        return Err(SchedulePrepareErrorV1::Replay(
            SimulationScheduleReplayErrorV1::ContextMismatch,
        ));
    }
    validate_decision_limit(record.decisions.len(), limits)?;
    if record.coverage.decisions != record.decisions.len() as u64
        || record.coverage.workgroups != plan.workgroups()
        || record.decisions.is_empty()
    {
        return Err(SchedulePrepareErrorV1::Replay(
            SimulationScheduleReplayErrorV1::InvalidRecordCoverage,
        ));
    }
    let valid_kind = matches!(
        (record.schedule, record.seed),
        (
            SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
            None
        ) | (
            SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1,
            Some(_)
        )
    );
    if !valid_kind
        || transcript_identity(
            record.context_identity,
            record.schedule,
            record.seed,
            record.coverage,
        ) != record.transcript_identity
        || record_integrity(record.transcript_identity, &record.decisions)
            != record.record_integrity
    {
        return Err(SchedulePrepareErrorV1::Replay(
            SimulationScheduleReplayErrorV1::TranscriptIntegrity,
        ));
    }
    Ok(())
}

pub(crate) fn schedule_context_identity(
    identity: SimulationKernelIrIdentityV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
) -> [u8; 32] {
    schedule_context_identity_configured(identity, request, None, target, limits)
}

pub(crate) fn schedule_context_identity_with_dynamic(
    identity: SimulationKernelIrIdentityV1,
    request: &SimulationRequestV1,
    dynamic: DynamicWorkgroupMemoryRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
) -> [u8; 32] {
    schedule_context_identity_configured(identity, request, Some(dynamic), target, limits)
}

fn schedule_context_identity_configured(
    identity: SimulationKernelIrIdentityV1,
    request: &SimulationRequestV1,
    dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    let write_only = request.arguments.iter().any(|argument| match argument {
        SimulationArgumentV1::Scalar(_) => false,
        SimulationArgumentV1::Buffer(buffer) => buffer.access() == AccessMode::WriteOnly,
        SimulationArgumentV1::BufferView(view) => view.access() == AccessMode::WriteOnly,
    }) || request
        .shared_buffers
        .iter()
        .any(|shared| shared.buffer.access() == AccessMode::WriteOnly);
    let versioned_context = identity.wire_version() != 7;
    if let Some(dynamic) = dynamic {
        hash.update(DYNAMIC_CONTEXT_DOMAIN_V1);
        hash.update(identity.wire_version().to_le_bytes());
        hash.update([u8::from(write_only)]);
        hash.update(dynamic.byte_extent().to_le_bytes());
    } else {
        hash.update(match (versioned_context, write_only) {
            (false, false) => CONTEXT_DOMAIN_V1,
            (false, true) => CONTEXT_DOMAIN_V2,
            (true, false) => CONTEXT_DOMAIN_V3,
            (true, true) => CONTEXT_DOMAIN_V4,
        });
    }
    if dynamic.is_none() && versioned_context {
        hash.update(identity.wire_version().to_le_bytes());
    }
    hash.update(identity.digest());
    hash.update(identity.canonical_length().to_le_bytes());
    hash_bytes(&mut hash, request.kernel.as_str().as_bytes());
    for value in request.grid.0 {
        hash.update(value.to_le_bytes());
    }
    for value in request.workgroup.0 {
        hash.update(value.to_le_bytes());
    }
    hash.update([match request.events {
        EventPolicyV1::Disabled => 0,
        EventPolicyV1::Enabled => 1,
    }]);
    hash.update([match target.index_width() {
        IndexWidthV1::Bits32 => 32,
        IndexWidthV1::Bits64 => 64,
    }]);
    hash_limits(&mut hash, limits);
    hash.update((request.arguments.len() as u64).to_le_bytes());
    for argument in &request.arguments {
        match argument {
            SimulationArgumentV1::Scalar(value) => {
                hash.update([0, scalar_tag(value.ty())]);
                hash.update(value.bits().to_le_bytes());
            }
            SimulationArgumentV1::Buffer(buffer) => {
                hash.update([1]);
                hash_buffer(&mut hash, buffer);
            }
            SimulationArgumentV1::BufferView(view) => {
                hash.update([2]);
                hash.update(view.backing().0.to_le_bytes());
                hash.update([scalar_tag(view.element()), access_tag(view.access())]);
                hash.update(view.alignment().to_le_bytes());
                hash.update((view.byte_offset() as u64).to_le_bytes());
                hash.update((view.elements() as u64).to_le_bytes());
            }
        }
    }
    hash.update((request.shared_buffers.len() as u64).to_le_bytes());
    for shared in &request.shared_buffers {
        hash.update(shared.id.0.to_le_bytes());
        hash_buffer(&mut hash, &shared.buffer);
    }
    hash.finalize().into()
}

fn transcript_identity(
    context_identity: [u8; 32],
    schedule: SimulationScheduleIdentityV1,
    seed: Option<u64>,
    coverage: SimulationScheduleCoverageV1,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(TRANSCRIPT_DOMAIN_V1);
    hash.update(context_identity);
    hash.update([schedule_tag(schedule)]);
    match seed {
        Some(seed) => {
            hash.update([1]);
            hash.update(seed.to_le_bytes());
        }
        None => hash.update([0]),
    }
    hash.update(coverage.decisions.to_le_bytes());
    hash.update(coverage.workgroups.to_le_bytes());
    hash.update(coverage.barrier_releases.to_le_bytes());
    hash.finalize().into()
}

fn record_integrity(
    transcript_identity: [u8; 32],
    decisions: &[SimulationScheduleDecisionV1],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(RECORD_INTEGRITY_DOMAIN_V1);
    hash.update(transcript_identity);
    hash.update((decisions.len() as u64).to_le_bytes());
    for decision in decisions {
        for coordinate in decision.workgroup {
            hash.update(coordinate.to_le_bytes());
        }
        hash.update(decision.phase.to_le_bytes());
        for coordinate in decision.local {
            hash.update(coordinate.to_le_bytes());
        }
    }
    hash.finalize().into()
}

fn hash_buffer(hash: &mut Sha256, buffer: &BufferArgumentV1) {
    hash.update([scalar_tag(buffer.element()), access_tag(buffer.access())]);
    hash.update(buffer.alignment().to_le_bytes());
    hash_bytes(hash, buffer.bytes());
    hash.update((buffer.initialized().len() as u64).to_le_bytes());
    for initialized in buffer.initialized() {
        hash.update([u8::from(*initialized)]);
    }
}

fn hash_bytes(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn hash_limits(hash: &mut Sha256, limits: SimulationLimitsV1) {
    for value in [
        limits.max_canonical_bytes as u64,
        limits.max_reachable_functions as u64,
        limits.max_reachable_operations as u64,
        limits.max_invocations,
        limits.max_workgroups,
        limits.max_scheduled_slots,
        limits.max_steps,
        limits.max_call_depth as u64,
        limits.max_ssa_values as u64,
        limits.max_allocations as u64,
        limits.max_allocation_bytes as u64,
        limits.max_total_bytes as u64,
        limits.max_resident_bytes as u64,
        limits.max_events,
        limits.max_memory_access_records as u64,
    ] {
        hash.update(value.to_le_bytes());
    }
}

const fn access_tag(access: AccessMode) -> u8 {
    match access {
        AccessMode::ReadOnly => 0,
        AccessMode::ReadWrite => 1,
        AccessMode::WriteOnly => 2,
    }
}

const fn schedule_tag(schedule: SimulationScheduleIdentityV1) -> u8 {
    match schedule {
        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxSerialV1 => 0,
        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1 => 1,
        SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1 => 2,
    }
}

const fn scalar_tag(scalar: ScalarType) -> u8 {
    match scalar {
        ScalarType::Bool => 0,
        ScalarType::I8 => 1,
        ScalarType::I16 => 2,
        ScalarType::I32 => 3,
        ScalarType::I64 => 4,
        ScalarType::I128 => 5,
        ScalarType::U8 => 6,
        ScalarType::U16 => 7,
        ScalarType::U32 => 8,
        ScalarType::U64 => 9,
        ScalarType::U128 => 10,
        ScalarType::Index => 11,
        ScalarType::F16 => 12,
        ScalarType::Bf16 => 13,
        ScalarType::F32 => 14,
        ScalarType::F64 => 15,
    }
}

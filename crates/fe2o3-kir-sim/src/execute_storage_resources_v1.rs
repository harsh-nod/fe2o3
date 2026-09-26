//! The ordinary interpreter and storage helpers debit the same step counter.

use super::*;
use crate::SimulationScheduleDecisionV1;
use std::cell::Cell;

pub(super) struct StorageAccountingV1 {
    steps: Cell<u64>,
    resident: Cell<usize>,
    peak: Cell<usize>,
    limits: SimulationLimitsV1,
}

impl StorageAccountingV1 {
    pub(super) fn new(limits: SimulationLimitsV1) -> Self {
        Self {
            steps: Cell::new(0),
            resident: Cell::new(0),
            peak: Cell::new(0),
            limits,
        }
    }

    pub(super) fn steps(&self) -> u64 {
        self.steps.get()
    }

    #[cfg(test)]
    pub(super) fn held(&self) -> usize {
        self.resident.get()
    }

    #[cfg(test)]
    pub(super) fn peak(&self) -> usize {
        self.peak.get()
    }

    pub(super) fn charge(&self, count: usize) -> Result<(), SimulationExecutionErrorKindV1> {
        let next = self
            .steps
            .get()
            .checked_add(u64::try_from(count).unwrap_or(u64::MAX))
            .filter(|next| *next <= self.limits.max_steps)
            .ok_or(SimulationExecutionErrorKindV1::StepLimit {
                limit: self.limits.max_steps,
            })?;
        self.steps.set(next);
        Ok(())
    }

    pub(super) fn hold(&self, bytes: usize) -> Result<(), SimulationExecutionErrorKindV1> {
        let next = self.resident.get().checked_add(bytes).ok_or(
            SimulationExecutionErrorKindV1::StorageResidentLimit {
                actual: usize::MAX,
                limit: self.limits.max_resident_bytes,
            },
        )?;
        if next > self.limits.max_resident_bytes {
            return Err(SimulationExecutionErrorKindV1::StorageResidentLimit {
                actual: next,
                limit: self.limits.max_resident_bytes,
            });
        }
        self.resident.set(next);
        self.peak.set(self.peak.get().max(next));
        Ok(())
    }

    pub(super) fn release(&self, bytes: usize) {
        self.resident.set(
            self.resident
                .get()
                .checked_sub(bytes)
                .expect("storage reservation is released once after its backing is dropped"),
        );
    }

    pub(super) fn temporary(
        &self,
        bytes: usize,
    ) -> Result<StorageReservationV1<'_>, SimulationExecutionErrorKindV1> {
        self.hold(bytes)?;
        Ok(StorageReservationV1 {
            accounting: self,
            bytes,
        })
    }
}

pub(super) struct StorageReservationV1<'a> {
    accounting: &'a StorageAccountingV1,
    bytes: usize,
}

impl Drop for StorageReservationV1<'_> {
    fn drop(&mut self) {
        self.accounting.release(self.bytes);
    }
}

pub(super) fn storage_bytes_v1<T>(count: usize) -> Result<usize, SimulationExecutionErrorKindV1> {
    count
        .checked_mul(size_of::<T>())
        .ok_or(SimulationExecutionErrorKindV1::StorageResidentLimit {
            actual: usize::MAX,
            limit: usize::MAX,
        })
}

pub(super) fn storage_growth_headers_v1<T>() -> usize {
    // Old/new/retired Vec slots; try_reserve's real return/caller pair; three
    // checked-capacity return/caller pairs; four hold/charge/result pairs;
    // and temporary reservation construction, return, and caller slots.
    3 * size_of::<Vec<T>>()
        + 2 * size_of::<Result<(), std::collections::TryReserveError>>()
        + 6 * size_of::<Result<usize, SimulationExecutionErrorKindV1>>()
        + 8 * size_of::<Result<(), SimulationExecutionErrorKindV1>>()
        + 3 * size_of::<StorageReservationV1<'_>>()
        + 2 * size_of::<Result<StorageReservationV1<'_>, SimulationExecutionErrorKindV1>>()
}

pub(super) fn storage_vector_headers_v1() -> usize {
    // ReadValue owns the decoding Vec until it moves into the retained arena.
    // Include the constructed arena row, decoded-index return/caller slots,
    // and the growth call's result pair independently of retained backing.
    size_of::<Vec<ScalarBitsV1>>()
        + size_of::<StorageVectorV1>()
        + 2 * size_of::<Result<usize, SimulationExecutionErrorV1>>()
        + 2 * size_of::<Result<(), SimulationExecutionErrorKindV1>>()
}

pub(super) fn storage_reserve_v1<T>(
    rows: &mut Vec<T>,
    minimum: usize,
    accounting: &StorageAccountingV1,
) -> Result<(), SimulationExecutionErrorKindV1> {
    if minimum <= rows.capacity() {
        return Ok(());
    }
    let previous = storage_bytes_v1::<T>(rows.capacity())?;
    let requested = minimum.max(rows.capacity().checked_mul(2).unwrap_or(minimum));
    let _headers = accounting.temporary(storage_growth_headers_v1::<T>())?;
    let predicted = storage_bytes_v1::<T>(requested)?;
    accounting.hold(predicted)?;
    let mut replacement = Vec::new();
    if replacement.try_reserve_exact(requested).is_err() {
        accounting.release(predicted);
        return Err(SimulationExecutionErrorKindV1::AllocationFailure);
    }
    let actual = match storage_bytes_v1::<T>(replacement.capacity()) {
        Ok(actual) => actual,
        Err(error) => {
            drop(replacement);
            accounting.release(predicted);
            return Err(error);
        }
    };
    if let Err(error) = accounting.hold(actual - predicted) {
        drop(replacement);
        accounting.release(predicted);
        return Err(error);
    }
    if let Err(error) = accounting.charge(rows.len()) {
        drop(replacement);
        accounting.release(actual);
        return Err(error);
    }
    replacement.append(rows);
    let retired = std::mem::replace(rows, replacement);
    drop(retired);
    accounting.release(previous);
    Ok(())
}

fn schedule_resident_error_v1(error: SimulationExecutionErrorKindV1) -> SchedulePrepareErrorV1 {
    match error {
        SimulationExecutionErrorKindV1::StorageResidentLimit { actual, limit } => {
            SchedulePrepareErrorV1::ResidentLimit { actual, limit }
        }
        _ => SchedulePrepareErrorV1::AllocationFailure,
    }
}

pub(super) fn reserve_schedule_decisions_v1(
    rows: &mut Vec<SimulationScheduleDecisionV1>,
    maximum: usize,
    accounting: &StorageAccountingV1,
) -> Result<(), SchedulePrepareErrorV1> {
    if rows.len() < rows.capacity() {
        return Ok(());
    }
    let minimum = rows
        .len()
        .checked_add(1)
        .ok_or(SchedulePrepareErrorV1::DecisionLimit {
            actual: usize::MAX,
            limit: maximum,
        })?;
    if minimum > maximum {
        return Err(SchedulePrepareErrorV1::DecisionLimit {
            actual: minimum,
            limit: maximum,
        });
    }
    let previous = storage_bytes_v1::<SimulationScheduleDecisionV1>(rows.capacity())
        .map_err(schedule_resident_error_v1)?;
    let requested = minimum.max(rows.capacity().saturating_mul(2).min(maximum));
    // This conservative header bound also covers the smaller scheduler error values.
    // Recorder bookkeeping does not debit semantic execution steps.
    let _headers = accounting
        .temporary(storage_growth_headers_v1::<SimulationScheduleDecisionV1>())
        .map_err(schedule_resident_error_v1)?;
    let predicted = storage_bytes_v1::<SimulationScheduleDecisionV1>(requested)
        .map_err(schedule_resident_error_v1)?;
    accounting
        .hold(predicted)
        .map_err(schedule_resident_error_v1)?;
    let mut replacement = Vec::new();
    if replacement.try_reserve_exact(requested).is_err() {
        accounting.release(predicted);
        return Err(SchedulePrepareErrorV1::AllocationFailure);
    }
    let actual = match storage_bytes_v1::<SimulationScheduleDecisionV1>(replacement.capacity()) {
        Ok(actual) => actual,
        Err(error) => {
            drop(replacement);
            accounting.release(predicted);
            return Err(schedule_resident_error_v1(error));
        }
    };
    if let Err(error) = accounting.hold(actual - predicted) {
        drop(replacement);
        accounting.release(predicted);
        return Err(schedule_resident_error_v1(error));
    }
    replacement.append(rows);
    let retired = std::mem::replace(rows, replacement);
    drop(retired);
    accounting.release(previous);
    Ok(())
}

#[derive(Debug)]
pub(crate) struct StorageExecutionCompletionV1<A = SimulationArgumentV1, B = SharedBufferV1> {
    pub(super) dynamic_workgroup_memory: Option<DynamicWorkgroupMemoryRequestV1>,
    pub(super) arguments: Vec<A>,
    pub(super) shared_buffers: Vec<B>,
    pub(super) invocations_executed: u64,
    pub(super) workgroups_visited: u64,
    pub(super) scheduled_slots_visited: u64,
    pub(super) steps_executed: u64,
    pub(super) events_emitted: u64,
    pub(super) schedule: SimulationScheduleIdentityV1,
    pub(super) schedule_transcript_identity: Option<[u8; 32]>,
    pub(super) schedule_coverage: SimulationScheduleCoverageV1,
    pub(super) supplemental: Vec<SimulationSupplementalV1>,
    pub(super) conflict_assessment: SimulationConflictAssessmentV1,
}

impl StorageExecutionCompletionV1 {
    pub(super) fn canonical(
        self,
        identity: SimulationKernelIrIdentityV1,
    ) -> Result<SimulationExecutionV1, SimulationExecutionErrorV1> {
        let schedule_transcript_identity = self.schedule_transcript_identity.ok_or_else(|| {
            top_level_error(storage_violation_v1(
                "private storage execution cannot become a canonical result",
            ))
        })?;
        Ok(SimulationExecutionV1 {
            identity,
            dynamic_workgroup_memory: self.dynamic_workgroup_memory,
            arguments: self.arguments,
            shared_buffers: self.shared_buffers,
            invocations_executed: self.invocations_executed,
            workgroups_visited: self.workgroups_visited,
            scheduled_slots_visited: self.scheduled_slots_visited,
            steps_executed: self.steps_executed,
            events_emitted: self.events_emitted,
            schedule: self.schedule,
            schedule_transcript_identity,
            schedule_coverage: self.schedule_coverage,
            supplemental: self.supplemental,
            conflict_assessment: self.conflict_assessment,
        })
    }
}

pub(super) enum StorageExecutionScheduleV1<'a> {
    Canonical(PreparedScheduleV1<'a>),
    Unidentified {
        decisions: u64,
        workgroups: u64,
        barrier_releases: u64,
    },
}

pub(super) struct StorageScheduleCompletionV1 {
    pub(super) identity: SimulationScheduleIdentityV1,
    pub(super) transcript_identity: Option<[u8; 32]>,
    pub(super) coverage: SimulationScheduleCoverageV1,
    pub(super) records: Vec<SimulationScheduleRecordV1>,
}

impl<'a> StorageExecutionScheduleV1<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        request: Option<ExecutionScheduleRequestV1<'a>>,
        identity: Option<SimulationKernelIrIdentityV1>,
        simulation: &SimulationRequestV1,
        dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        plan: &SimulationPlanV1,
        participants: usize,
        resident_offset: usize,
    ) -> Result<Self, SchedulePrepareErrorV1> {
        Self::prepare_inputs_v29(request, identity, Some(simulation), dynamic, target, limits, plan, participants, resident_offset)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_inputs_v29(
        request: Option<ExecutionScheduleRequestV1<'a>>,
        identity: Option<SimulationKernelIrIdentityV1>,
        simulation: Option<&SimulationRequestV1>,
        dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        plan: &SimulationPlanV1,
        participants: usize,
        resident_offset: usize,
    ) -> Result<Self, SchedulePrepareErrorV1> {
        match (identity, simulation) {
            (Some(identity), Some(simulation)) => PreparedScheduleV1::prepare(
                request,
                identity,
                simulation,
                dynamic,
                target,
                limits,
                plan,
                participants,
                resident_offset,
            )
            .map(Self::Canonical),
            (None, _) if request.is_none() => Ok(Self::Unidentified {
                decisions: 0,
                workgroups: 0,
                barrier_releases: 0,
            }),
            _ => Err(SchedulePrepareErrorV1::Replay(
                SimulationScheduleReplayErrorV1::CoverageMismatch,
            )),
        }
    }

    pub(super) fn initial_resident_bytes(&self) -> usize {
        match self {
            Self::Canonical(schedule) => schedule.initial_resident_bytes(),
            Self::Unidentified { .. } => 0,
        }
    }

    pub(super) fn identity(&self) -> SimulationScheduleIdentityV1 {
        match self {
            Self::Canonical(schedule) => schedule.identity(),
            Self::Unidentified { .. } => {
                SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1
            }
        }
    }

    pub(super) fn uses_canonical_order(&self) -> bool {
        match self {
            Self::Canonical(schedule) => schedule.uses_canonical_order(),
            Self::Unidentified { .. } => true,
        }
    }

    pub(super) fn current_decision(&self) -> u64 {
        match self {
            Self::Canonical(schedule) => schedule.current_decision(),
            Self::Unidentified { decisions, .. } => *decisions,
        }
    }

    pub(super) fn begin_workgroup(&mut self) {
        match self {
            Self::Canonical(schedule) => schedule.begin_workgroup(),
            Self::Unidentified { workgroups, .. } => {
                *workgroups = workgroups
                    .checked_add(1)
                    .expect("preflighted workgroup count fits u64");
            }
        }
    }

    pub(super) fn selected(
        &mut self,
        invocation: SimulationInvocationV1,
        phase: u64,
        accounting: &StorageAccountingV1,
    ) -> Result<(), SchedulePrepareErrorV1> {
        match self {
            Self::Canonical(schedule) => schedule.selected(invocation, phase, |rows, maximum| {
                reserve_schedule_decisions_v1(rows, maximum, accounting)
            }),
            Self::Unidentified { decisions, .. } => {
                *decisions =
                    decisions
                        .checked_add(1)
                        .ok_or(SchedulePrepareErrorV1::DecisionLimit {
                            actual: usize::MAX,
                            limit: usize::MAX,
                        })?;
                Ok(())
            }
        }
    }

    pub(super) fn barrier_released(&mut self) {
        match self {
            Self::Canonical(schedule) => schedule.barrier_released(),
            Self::Unidentified {
                barrier_releases, ..
            } => {
                *barrier_releases = barrier_releases
                    .checked_add(1)
                    .expect("barrier releases are bounded by the finite execution step limit");
            }
        }
    }

    pub(super) fn take_order(
        &mut self,
        participants: usize,
        invocation: impl Fn(usize) -> SimulationInvocationV1,
        runnable: impl Fn(usize) -> bool,
        workgroup: [u64; 3],
        phase: u64,
    ) -> Result<Vec<usize>, SimulationScheduleReplayErrorV1> {
        match self {
            Self::Canonical(schedule) => {
                schedule.take_order(participants, invocation, runnable, workgroup, phase)
            }
            Self::Unidentified { .. } => Err(SimulationScheduleReplayErrorV1::CoverageMismatch),
        }
    }

    pub(super) fn restore_order(&mut self, order: Vec<usize>) {
        match self {
            Self::Canonical(schedule) => schedule.restore_order(order),
            Self::Unidentified { .. } => {
                unreachable!("unidentified execution only uses the existing canonical-order branch")
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish(
        self,
        expected_workgroups: u64,
        identity: Option<SimulationKernelIrIdentityV1>,
        simulation: &SimulationRequestV1,
        dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
    ) -> Result<StorageScheduleCompletionV1, SimulationScheduleReplayErrorV1> {
        self.finish_inputs_v29(expected_workgroups, identity, Some(simulation), dynamic, target, limits)
    }

    pub(super) fn finish_inputs_v29(
        self,
        expected_workgroups: u64,
        identity: Option<SimulationKernelIrIdentityV1>,
        simulation: Option<&SimulationRequestV1>,
        dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
    ) -> Result<StorageScheduleCompletionV1, SimulationScheduleReplayErrorV1> {
        match (self, identity, simulation) {
            (Self::Canonical(schedule), Some(identity), Some(simulation)) => {
                let result = schedule.finish(
                    expected_workgroups,
                    identity,
                    simulation,
                    dynamic,
                    target,
                    limits,
                )?;
                Ok(StorageScheduleCompletionV1 {
                    identity: result.identity,
                    transcript_identity: Some(result.transcript_identity),
                    coverage: result.coverage,
                    records: result.records,
                })
            }
            (
                Self::Unidentified {
                    decisions,
                    workgroups,
                    barrier_releases,
                },
                None,
                _,
            ) => {
                let coverage = SimulationScheduleCoverageV1::from_completed_counts(
                    decisions,
                    workgroups,
                    barrier_releases,
                    expected_workgroups,
                )?;
                Ok(StorageScheduleCompletionV1 {
                    identity: SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
                    transcript_identity: None,
                    coverage,
                    records: Vec::new(),
                })
            }
            _ => Err(SimulationScheduleReplayErrorV1::CoverageMismatch),
        }
    }
}

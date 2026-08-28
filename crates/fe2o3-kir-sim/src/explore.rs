use std::error::Error;
use std::fmt;
use std::mem::size_of;

use crate::{
    AdmittedSimulationModuleV1, SimulationErrorV1, SimulationExecutionErrorKindV1,
    SimulationExecutionErrorV1, SimulationInvocationV1, SimulationLimitsV1,
    SimulationMemoryConflictV1, SimulationRaceAssessmentV1, SimulationRequestV1,
    SimulationScheduleRecordV1, SimulationScheduleRequestV1, SimulationSiteV1, SimulationTargetV1,
};

/// Hard upper bound on schedules attempted by one exploration call.
pub const MAX_EXPLORATION_SCHEDULES_V1: usize = 4_096;
/// Hard upper bound on schedule decisions retained across exploration witnesses.
pub const MAX_EXPLORATION_RETAINED_DECISIONS_V1: usize = 4 * 1024 * 1024;

/// Bounded deterministic seeded-interleaving exploration request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationExplorationRequestV1 {
    first_seed: u64,
    max_schedules: usize,
    max_decisions_per_schedule: usize,
    max_retained_decisions: usize,
}

impl SimulationExplorationRequestV1 {
    pub fn new(
        first_seed: u64,
        max_schedules: usize,
        max_decisions_per_schedule: usize,
        max_retained_decisions: usize,
    ) -> Result<Self, SimulationExplorationRequestErrorV1> {
        if max_schedules == 0 {
            return Err(SimulationExplorationRequestErrorV1::Zero("max_schedules"));
        }
        if max_schedules > MAX_EXPLORATION_SCHEDULES_V1 {
            return Err(SimulationExplorationRequestErrorV1::AboveHardCap(
                "max_schedules",
            ));
        }
        if max_decisions_per_schedule == 0 {
            return Err(SimulationExplorationRequestErrorV1::Zero(
                "max_decisions_per_schedule",
            ));
        }
        if max_decisions_per_schedule > crate::MAX_SCHEDULE_DECISIONS_V1 {
            return Err(SimulationExplorationRequestErrorV1::AboveHardCap(
                "max_decisions_per_schedule",
            ));
        }
        if max_retained_decisions == 0 {
            return Err(SimulationExplorationRequestErrorV1::Zero(
                "max_retained_decisions",
            ));
        }
        if max_retained_decisions > MAX_EXPLORATION_RETAINED_DECISIONS_V1 {
            return Err(SimulationExplorationRequestErrorV1::AboveHardCap(
                "max_retained_decisions",
            ));
        }
        Ok(Self {
            first_seed,
            max_schedules,
            max_decisions_per_schedule,
            max_retained_decisions,
        })
    }

    pub const fn first_seed(self) -> u64 {
        self.first_seed
    }

    pub const fn max_schedules(self) -> usize {
        self.max_schedules
    }

    pub const fn max_decisions_per_schedule(self) -> usize {
        self.max_decisions_per_schedule
    }

    pub const fn max_retained_decisions(self) -> usize {
        self.max_retained_decisions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationExplorationRequestErrorV1 {
    Zero(&'static str),
    AboveHardCap(&'static str),
}

impl fmt::Display for SimulationExplorationRequestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid simulation exploration request: {self:?}"
        )
    }
}

impl Error for SimulationExplorationRequestErrorV1 {}

/// One exact replayable schedule retained as a reducer/debugger seed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationExplorationWitnessV1 {
    seed: u64,
    schedule: SimulationScheduleRecordV1,
    assessment: SimulationRaceAssessmentV1,
}

impl SimulationExplorationWitnessV1 {
    pub const fn seed(&self) -> u64 {
        self.seed
    }

    pub const fn schedule(&self) -> &SimulationScheduleRecordV1 {
        &self.schedule
    }

    pub const fn assessment(&self) -> &SimulationRaceAssessmentV1 {
        &self.assessment
    }
}

/// First typed dynamic failure found while sweeping seeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationExplorationFailureV1 {
    pub seed: u64,
    pub invocation: Option<SimulationInvocationV1>,
    pub site: Option<SimulationSiteV1>,
    pub kind: SimulationExecutionErrorKindV1,
}

/// Bounded exploration summary. Budget exhaustion is explicit and never proves absence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationExplorationV1 {
    attempted: usize,
    completed: usize,
    races_observed: usize,
    no_races_observed: usize,
    incomplete_assessments: usize,
    failures: usize,
    retained_decisions: usize,
    requested_seed_budget_consumed: bool,
    witness_retention_exhausted: bool,
    first_race: Option<SimulationExplorationWitnessV1>,
    first_no_race: Option<SimulationExplorationWitnessV1>,
    first_incomplete: Option<SimulationExplorationWitnessV1>,
    first_failure: Option<SimulationExplorationFailureV1>,
}

impl SimulationExplorationV1 {
    pub const fn attempted(&self) -> usize {
        self.attempted
    }
    pub const fn completed(&self) -> usize {
        self.completed
    }
    pub const fn races_observed(&self) -> usize {
        self.races_observed
    }
    pub const fn no_races_observed(&self) -> usize {
        self.no_races_observed
    }
    pub const fn incomplete_assessments(&self) -> usize {
        self.incomplete_assessments
    }
    pub const fn failures(&self) -> usize {
        self.failures
    }
    pub const fn retained_decisions(&self) -> usize {
        self.retained_decisions
    }
    /// Whether every seed in the caller's requested bounded interval was attempted.
    ///
    /// This does not mean that the possible schedule space was exhausted.
    pub const fn requested_seed_budget_consumed(&self) -> bool {
        self.requested_seed_budget_consumed
    }
    pub const fn witness_retention_exhausted(&self) -> bool {
        self.witness_retention_exhausted
    }
    pub const fn first_race(&self) -> Option<&SimulationExplorationWitnessV1> {
        self.first_race.as_ref()
    }
    pub const fn first_no_race(&self) -> Option<&SimulationExplorationWitnessV1> {
        self.first_no_race.as_ref()
    }
    pub const fn first_incomplete(&self) -> Option<&SimulationExplorationWitnessV1> {
        self.first_incomplete.as_ref()
    }
    pub const fn first_failure(&self) -> Option<&SimulationExplorationFailureV1> {
        self.first_failure.as_ref()
    }
}

impl AdmittedSimulationModuleV1 {
    /// Sweeps a contiguous, wrapping seed interval and retains at most one witness per class.
    #[allow(clippy::result_large_err)]
    pub fn explore_seeded_schedules(
        &self,
        simulation: &SimulationRequestV1,
        target: SimulationTargetV1,
        limits: SimulationLimitsV1,
        request: SimulationExplorationRequestV1,
    ) -> Result<SimulationExplorationV1, SimulationErrorV1> {
        let mut result = SimulationExplorationV1 {
            attempted: 0,
            completed: 0,
            races_observed: 0,
            no_races_observed: 0,
            incomplete_assessments: 0,
            failures: 0,
            retained_decisions: 0,
            requested_seed_budget_consumed: false,
            witness_retention_exhausted: false,
            first_race: None,
            first_no_race: None,
            first_incomplete: None,
            first_failure: None,
        };
        // The exploration summary itself remains live around every scheduled
        // execution; per-run accounting charges SimulationExecutionV1 instead.
        let mut retained_result_bytes = size_of::<SimulationExplorationV1>();
        let mut first_failure_seen = false;
        for offset in 0..request.max_schedules {
            let seed = request.first_seed.wrapping_add(offset as u64);
            result.attempted += 1;
            match self.simulate_scheduled_with_resident_offset(
                simulation,
                target,
                limits,
                SimulationScheduleRequestV1::RecordSeeded {
                    seed,
                    max_decisions: request.max_decisions_per_schedule,
                },
                retained_result_bytes,
            ) {
                Ok(execution) => {
                    result.completed += 1;
                    let (schedule, assessment) = execution.into_schedule_and_race();
                    let schedule = schedule.ok_or({
                        SimulationErrorV1::Execution(SimulationExecutionErrorV1 {
                            invocation: None,
                            site: None,
                            kind: SimulationExecutionErrorKindV1::InternalInvariant(
                                "seeded exploration schedule record",
                            ),
                            observation_failure: None,
                        })
                    })?;
                    let slot = match &assessment {
                        SimulationRaceAssessmentV1::RacesObserved { .. } => {
                            result.races_observed += 1;
                            &mut result.first_race
                        }
                        SimulationRaceAssessmentV1::NoRacesObserved { .. } => {
                            result.no_races_observed += 1;
                            &mut result.first_no_race
                        }
                        SimulationRaceAssessmentV1::Incomplete { .. } => {
                            result.incomplete_assessments += 1;
                            &mut result.first_incomplete
                        }
                    };
                    if slot.is_none() {
                        let decisions = schedule.decisions().len();
                        let witness_bytes = schedule.retained_heap_bytes().and_then(|bytes| {
                            race_assessment_retained_bytes(&assessment)
                                .and_then(|assessment| bytes.checked_add(assessment))
                        });
                        let retained_after = witness_bytes
                            .and_then(|bytes| retained_result_bytes.checked_add(bytes));
                        if result
                            .retained_decisions
                            .checked_add(decisions)
                            .is_some_and(|retained| retained <= request.max_retained_decisions)
                            && retained_after
                                .is_some_and(|retained| retained <= limits.max_resident_bytes)
                        {
                            result.retained_decisions += decisions;
                            retained_result_bytes =
                                retained_after.expect("checked retained exploration witness bytes");
                            *slot = Some(SimulationExplorationWitnessV1 {
                                seed,
                                schedule,
                                assessment,
                            });
                        } else {
                            result.witness_retention_exhausted = true;
                        }
                    }
                }
                Err(SimulationErrorV1::Preflight(error)) => {
                    return Err(SimulationErrorV1::Preflight(error));
                }
                Err(SimulationErrorV1::Execution(error)) => {
                    result.failures += 1;
                    if !first_failure_seen {
                        first_failure_seen = true;
                        let failure = SimulationExplorationFailureV1 {
                            seed,
                            invocation: error.invocation,
                            site: error.site,
                            kind: error.kind,
                        };
                        let retained_after = exploration_failure_retained_bytes(&failure)
                            .and_then(|bytes| retained_result_bytes.checked_add(bytes));
                        if retained_after
                            .is_some_and(|retained| retained <= limits.max_resident_bytes)
                        {
                            retained_result_bytes =
                                retained_after.expect("checked retained exploration failure bytes");
                            result.first_failure = Some(failure);
                        } else {
                            result.witness_retention_exhausted = true;
                        }
                    }
                }
            }
        }
        result.requested_seed_budget_consumed = true;
        Ok(result)
    }
}

fn race_assessment_retained_bytes(assessment: &SimulationRaceAssessmentV1) -> Option<usize> {
    match assessment {
        SimulationRaceAssessmentV1::NoRacesObserved {
            first_ordered_conflict,
        } => first_ordered_conflict.as_ref().map_or(Some(0), |ordered| {
            conflict_retained_bytes(&ordered.conflict)
        }),
        SimulationRaceAssessmentV1::RacesObserved {
            first,
            first_ordered_conflict,
            ..
        }
        | SimulationRaceAssessmentV1::Incomplete {
            first: Some(first),
            first_ordered_conflict,
            ..
        } => conflict_retained_bytes(&first.conflict).and_then(|bytes| {
            first_ordered_conflict
                .as_ref()
                .map_or(Some(bytes), |ordered| {
                    conflict_retained_bytes(&ordered.conflict)
                        .and_then(|ordered_bytes| bytes.checked_add(ordered_bytes))
                })
        }),
        SimulationRaceAssessmentV1::Incomplete {
            first: None,
            first_ordered_conflict,
            ..
        } => first_ordered_conflict.as_ref().map_or(Some(0), |ordered| {
            conflict_retained_bytes(&ordered.conflict)
        }),
    }
}

fn conflict_retained_bytes(conflict: &SimulationMemoryConflictV1) -> Option<usize> {
    conflict
        .earlier_site
        .function
        .retained_capacity_bytes()
        .checked_add(conflict.later_site.function.retained_capacity_bytes())
}

fn exploration_failure_retained_bytes(failure: &SimulationExplorationFailureV1) -> Option<usize> {
    let site = failure
        .site
        .as_ref()
        .map_or(0, |site| site.function.retained_capacity_bytes());
    let kind = failure.kind.retained_heap_bytes();
    site.checked_add(kind)
}

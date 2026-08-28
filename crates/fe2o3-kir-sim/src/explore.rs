use std::error::Error;
use std::fmt;

use crate::{
    AdmittedSimulationModuleV1, SimulationErrorV1, SimulationExecutionErrorKindV1,
    SimulationExecutionErrorV1, SimulationInvocationV1, SimulationLimitsV1,
    SimulationRaceAssessmentV1, SimulationRequestV1, SimulationScheduleRecordV1,
    SimulationScheduleRequestV1, SimulationSiteV1, SimulationTargetV1,
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
        for offset in 0..request.max_schedules {
            let seed = request.first_seed.wrapping_add(offset as u64);
            result.attempted += 1;
            match self.simulate_scheduled(
                simulation,
                target,
                limits,
                SimulationScheduleRequestV1::RecordSeeded {
                    seed,
                    max_decisions: request.max_decisions_per_schedule,
                },
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
                        if result.retained_decisions.saturating_add(decisions)
                            <= request.max_retained_decisions
                        {
                            result.retained_decisions += decisions;
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
                    if result.first_failure.is_none() {
                        result.first_failure = Some(SimulationExplorationFailureV1 {
                            seed,
                            invocation: error.invocation,
                            site: error.site,
                            kind: error.kind,
                        });
                    }
                }
            }
        }
        result.requested_seed_budget_consumed = true;
        Ok(result)
    }
}

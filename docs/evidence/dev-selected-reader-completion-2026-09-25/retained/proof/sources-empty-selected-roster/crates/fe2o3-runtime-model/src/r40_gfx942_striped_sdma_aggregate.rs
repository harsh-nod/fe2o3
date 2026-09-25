//! Independent executable R40 model for gfx942 striped-SDMA capacity and
//! atomic whole-submission aggregate completion.
//!
//! The capacity model has exactly two SDMA engines and eight queues per engine.
//! A combined plan reserves one directional queue on each engine and admits an
//! even striped count from 2 through 14. A standalone plan admits an even
//! striped count from 2 through 16. Queue IDs are distinct only within the
//! supplied session, and striped queues are placed evenly across the engines.
//!
//! The aggregate model owns one bounded submission. Before inspecting any
//! completion status it validates the exact queue plan, shard roster, immutable
//! completion-validation roster, tickets, request indices, queue slots, queue
//! IDs, generations, and closed-currentness marker. Construction prepares that
//! roster and an empty completed-output allocation before the abstract
//! publication boundary. It then scans statuses in request order. Pending and
//! timeout return the exact whole submission, including both preparations,
//! without retirement. An error stops the scan and returns terminal custody. If
//! all statuses are ready, every retirement entry is preflighted before ordered
//! shards are moved into the already allocated output without growing it.
//! A contracted post-retirement model-retake failure retains that entire moved
//! output in a distinct terminal owner and exposes no normal completed result.
//!
//! All values are caller-constructed mathematical inputs. This module performs
//! no I/O and does not refine production Rust, KFD, HSA, HIP, queue ioctls,
//! packets, atomics, clocks, firmware, hardware, progress, parity, or
//! performance. The retake branch is likewise abstract and does not model a
//! production reconstruction failure.

use alloc::vec::Vec;

use crate::r46_gfx942_striped_sdma_tail_wait::R46AllReadyFullAuditWitnessV1;

pub const R40_GFX942_SDMA_ENGINE_COUNT_V1: u8 = 2;
pub const R40_GFX942_SDMA_QUEUES_PER_ENGINE_V1: u8 = 8;
pub const R40_GFX942_COMBINED_MIN_STRIPED_QUEUES_V1: u8 = 2;
pub const R40_GFX942_COMBINED_MAX_STRIPED_QUEUES_V1: u8 = 14;
pub const R40_GFX942_STANDALONE_MIN_STRIPED_QUEUES_V1: u8 = 2;
pub const R40_GFX942_STANDALONE_MAX_STRIPED_QUEUES_V1: u8 = 16;
pub const R40_GFX942_REQUESTS_PER_STRIPED_QUEUE_V1: u16 = 63;
pub const R40_GFX942_COMBINED_MAX_REQUESTS_V1: u16 = 882;
pub const R40_GFX942_STANDALONE_MAX_REQUESTS_V1: u16 = 1008;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R40Gfx942SdmaPlanKindV1 {
    CombinedDirectionalStriped,
    StandaloneStriped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R40Gfx942SdmaQueueRoleV1 {
    Directional,
    Striped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R40Gfx942SdmaQueueBindingV1 {
    pub session_id: u64,
    pub queue_id: u32,
    pub engine_index: u8,
    pub role: R40Gfx942SdmaQueueRoleV1,
    pub role_index: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R40Gfx942SdmaQueuePlanV1 {
    pub session_id: u64,
    pub kind: R40Gfx942SdmaPlanKindV1,
    pub striped_queue_count: u8,
    pub queues: Vec<R40Gfx942SdmaQueueBindingV1>,
}

impl R40Gfx942SdmaQueuePlanV1 {
    pub fn create_model_only(
        session_id: u64,
        kind: R40Gfx942SdmaPlanKindV1,
        striped_queue_count: u8,
        queue_ids: &[u32],
    ) -> Result<Self, R40Gfx942SdmaPlanErrorV1> {
        validate_striped_count_model_only(kind, striped_queue_count)?;
        let directional_count = match kind {
            R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped => R40_GFX942_SDMA_ENGINE_COUNT_V1,
            R40Gfx942SdmaPlanKindV1::StandaloneStriped => 0,
        };
        let expected_count = usize::from(directional_count + striped_queue_count);
        if queue_ids.len() != expected_count {
            return Err(R40Gfx942SdmaPlanErrorV1::WrongQueueIdCount);
        }
        for (index, queue_id) in queue_ids.iter().enumerate() {
            if queue_ids[..index].contains(queue_id) {
                return Err(R40Gfx942SdmaPlanErrorV1::DuplicateSessionQueueId);
            }
        }

        let mut queues = Vec::with_capacity(expected_count);
        if kind == R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped {
            for engine_index in 0..R40_GFX942_SDMA_ENGINE_COUNT_V1 {
                queues.push(R40Gfx942SdmaQueueBindingV1 {
                    session_id,
                    queue_id: queue_ids[usize::from(engine_index)],
                    engine_index,
                    role: R40Gfx942SdmaQueueRoleV1::Directional,
                    role_index: engine_index,
                });
            }
        }
        for role_index in 0..striped_queue_count {
            let queue_index = usize::from(directional_count + role_index);
            queues.push(R40Gfx942SdmaQueueBindingV1 {
                session_id,
                queue_id: queue_ids[queue_index],
                engine_index: role_index % R40_GFX942_SDMA_ENGINE_COUNT_V1,
                role: R40Gfx942SdmaQueueRoleV1::Striped,
                role_index,
            });
        }

        let plan = Self {
            session_id,
            kind,
            striped_queue_count,
            queues,
        };
        if !plan.is_exact_model_only() {
            return Err(R40Gfx942SdmaPlanErrorV1::InvalidPlacement);
        }
        Ok(plan)
    }

    pub fn request_capacity_model_only(&self) -> u16 {
        u16::from(self.striped_queue_count) * R40_GFX942_REQUESTS_PER_STRIPED_QUEUE_V1
    }

    pub fn striped_queue_model_only(
        &self,
        striped_slot: u8,
    ) -> Option<R40Gfx942SdmaQueueBindingV1> {
        self.queues.iter().copied().find(|queue| {
            queue.role == R40Gfx942SdmaQueueRoleV1::Striped && queue.role_index == striped_slot
        })
    }

    pub fn queues_on_engine_model_only(&self, engine_index: u8) -> usize {
        self.queues
            .iter()
            .filter(|queue| queue.engine_index == engine_index)
            .count()
    }

    pub fn is_exact_model_only(&self) -> bool {
        if validate_striped_count_model_only(self.kind, self.striped_queue_count).is_err() {
            return false;
        }
        let directional_count = match self.kind {
            R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped => R40_GFX942_SDMA_ENGINE_COUNT_V1,
            R40Gfx942SdmaPlanKindV1::StandaloneStriped => 0,
        };
        if self.queues.len() != usize::from(directional_count + self.striped_queue_count) {
            return false;
        }
        for (index, queue) in self.queues.iter().enumerate() {
            if queue.session_id != self.session_id
                || queue.engine_index >= R40_GFX942_SDMA_ENGINE_COUNT_V1
                || self.queues[..index]
                    .iter()
                    .any(|prior| prior.queue_id == queue.queue_id)
            {
                return false;
            }
            let (expected_role, expected_role_index, expected_engine) =
                if index < usize::from(directional_count) {
                    (
                        R40Gfx942SdmaQueueRoleV1::Directional,
                        index as u8,
                        index as u8,
                    )
                } else {
                    let role_index = index as u8 - directional_count;
                    (
                        R40Gfx942SdmaQueueRoleV1::Striped,
                        role_index,
                        role_index % R40_GFX942_SDMA_ENGINE_COUNT_V1,
                    )
                };
            if queue.role != expected_role
                || queue.role_index != expected_role_index
                || queue.engine_index != expected_engine
            {
                return false;
            }
        }
        self.queues_on_engine_model_only(0) == self.queues_on_engine_model_only(1)
            && self.queues_on_engine_model_only(0)
                <= usize::from(R40_GFX942_SDMA_QUEUES_PER_ENGINE_V1)
    }
}

fn validate_striped_count_model_only(
    kind: R40Gfx942SdmaPlanKindV1,
    count: u8,
) -> Result<(), R40Gfx942SdmaPlanErrorV1> {
    let (minimum, maximum) = match kind {
        R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped => (
            R40_GFX942_COMBINED_MIN_STRIPED_QUEUES_V1,
            R40_GFX942_COMBINED_MAX_STRIPED_QUEUES_V1,
        ),
        R40Gfx942SdmaPlanKindV1::StandaloneStriped => (
            R40_GFX942_STANDALONE_MIN_STRIPED_QUEUES_V1,
            R40_GFX942_STANDALONE_MAX_STRIPED_QUEUES_V1,
        ),
    };
    if count < minimum || count > maximum {
        return Err(R40Gfx942SdmaPlanErrorV1::StripedCountOutOfRange);
    }
    if !count.is_multiple_of(R40_GFX942_SDMA_ENGINE_COUNT_V1) {
        return Err(R40Gfx942SdmaPlanErrorV1::StripedCountNotBalanced);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R40Gfx942SdmaPlanErrorV1 {
    StripedCountOutOfRange,
    StripedCountNotBalanced,
    WrongQueueIdCount,
    DuplicateSessionQueueId,
    InvalidPlacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R40AggregateTicketV1 {
    pub session_id: u64,
    pub submission_id: u64,
    pub request_index: u16,
    pub queue_slot: u8,
    pub queue_id: u32,
    pub queue_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R40AggregateShardV1 {
    pub ticket: R40AggregateTicketV1,
    pub payload_token: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R40AggregatePlanV1 {
    pub queue_plan: R40Gfx942SdmaQueuePlanV1,
    pub submission_id: u64,
    pub started_ns: u64,
    pub deadline_ns: u64,
    pub request_count: u16,
    pub queue_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R40AggregatePresentationV1 {
    pub plan: R40AggregatePlanV1,
    pub shards: Vec<R40AggregateShardV1>,
    pub completion_validation_roster: Vec<R40AggregateTicketV1>,
    pub currentness_closed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R40AggregateObservationStateV1 {
    Ready,
    Pending,
    Error { terminal_token: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R40AggregateObservationV1 {
    pub ticket: R40AggregateTicketV1,
    pub state: R40AggregateObservationStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R40AggregateCreateErrorV1 {
    InvalidQueuePlan,
    EmptySubmission,
    CapacityExceeded,
    ZeroQueueGeneration,
    InvalidDeadline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R40AggregateTerminalReasonV1 {
    InvalidObservationTime,
    PresentationSubstitution,
    CurrentnessNotClosed,
    PreparedStateLost,
    ObservationRosterMismatch,
    ObservationError { terminal_token: u64 },
    RetirementPreflightMismatch,
    RetirementPreflightFailed { request_index: u16 },
    ModelRetakeFailed { terminal_token: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R40AggregateModelRetakeV1 {
    Succeeds,
    Fails { terminal_token: u64 },
}

/// Move-only owner of one abstract whole aggregate submission.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R40AggregateSubmissionV1;
/// let owner: R40AggregateSubmissionV1 = todo!();
/// let duplicate = owner.clone();
/// # let _ = duplicate;
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R40AggregateSubmissionV1 {
    plan: R40AggregatePlanV1,
    shards: Vec<R40AggregateShardV1>,
    completion_validation_roster: Vec<R40AggregateTicketV1>,
    prepared_completed_output: Vec<R40AggregateShardV1>,
}

impl R40AggregateSubmissionV1 {
    pub fn new_model_only(
        queue_plan: R40Gfx942SdmaQueuePlanV1,
        submission_id: u64,
        started_ns: u64,
        deadline_ns: u64,
        request_count: u16,
        queue_generation: u64,
    ) -> Result<Self, R40AggregateCreateErrorV1> {
        if !queue_plan.is_exact_model_only() {
            return Err(R40AggregateCreateErrorV1::InvalidQueuePlan);
        }
        if request_count == 0 {
            return Err(R40AggregateCreateErrorV1::EmptySubmission);
        }
        if request_count > queue_plan.request_capacity_model_only() {
            return Err(R40AggregateCreateErrorV1::CapacityExceeded);
        }
        if queue_generation == 0 {
            return Err(R40AggregateCreateErrorV1::ZeroQueueGeneration);
        }
        if deadline_ns < started_ns {
            return Err(R40AggregateCreateErrorV1::InvalidDeadline);
        }

        let plan = R40AggregatePlanV1 {
            queue_plan,
            submission_id,
            started_ns,
            deadline_ns,
            request_count,
            queue_generation,
        };
        let mut shards = Vec::with_capacity(usize::from(request_count));
        for request_index in 0..request_count {
            let queue_slot = (request_index % u16::from(plan.queue_plan.striped_queue_count)) as u8;
            let queue = plan
                .queue_plan
                .striped_queue_model_only(queue_slot)
                .expect("an exact queue plan contains every striped slot");
            shards.push(R40AggregateShardV1 {
                ticket: R40AggregateTicketV1 {
                    session_id: plan.queue_plan.session_id,
                    submission_id,
                    request_index,
                    queue_slot,
                    queue_id: queue.queue_id,
                    queue_generation,
                },
                payload_token: u64::from(request_index) + 1,
            });
        }
        let completion_validation_roster = shards.iter().map(|shard| shard.ticket).collect();
        let prepared_completed_output = Vec::with_capacity(usize::from(request_count));
        Ok(Self {
            plan,
            shards,
            completion_validation_roster,
            prepared_completed_output,
        })
    }

    /// Constructs both the owner and its reusable validation presentation
    /// before either value crosses the abstract publication boundary.
    pub fn new_with_presentation_model_only(
        queue_plan: R40Gfx942SdmaQueuePlanV1,
        submission_id: u64,
        started_ns: u64,
        deadline_ns: u64,
        request_count: u16,
        queue_generation: u64,
    ) -> Result<(Self, R40AggregatePresentationV1), R40AggregateCreateErrorV1> {
        let submission = Self::new_model_only(
            queue_plan,
            submission_id,
            started_ns,
            deadline_ns,
            request_count,
            queue_generation,
        )?;
        let presentation = R40AggregatePresentationV1 {
            plan: submission.plan.clone(),
            shards: submission.shards.clone(),
            completion_validation_roster: submission.completion_validation_roster.clone(),
            currentness_closed: true,
        };
        Ok((submission, presentation))
    }

    pub fn plan_model_only(&self) -> &R40AggregatePlanV1 {
        &self.plan
    }

    pub fn shards_model_only(&self) -> &[R40AggregateShardV1] {
        &self.shards
    }

    pub fn completion_validation_roster_model_only(&self) -> &[R40AggregateTicketV1] {
        &self.completion_validation_roster
    }

    pub fn prepared_completed_output_capacity_model_only(&self) -> usize {
        self.prepared_completed_output.capacity()
    }

    pub fn prepared_completed_output_is_empty_model_only(&self) -> bool {
        self.prepared_completed_output.is_empty()
    }

    pub fn is_fully_prepared_model_only(&self) -> bool {
        self.completion_validation_roster.len() == self.shards.len()
            && self
                .completion_validation_roster
                .iter()
                .zip(&self.shards)
                .all(|(ticket, shard)| *ticket == shard.ticket)
            && self.prepared_completed_output.is_empty()
            && self.prepared_completed_output.capacity() >= self.shards.len()
    }

    pub fn poll_model_only(
        self,
        presentation: &R40AggregatePresentationV1,
        observations: &[R40AggregateObservationV1],
        now_ns: u64,
        retirement_preflight: &[bool],
    ) -> R40AggregatePollOutcomeV1 {
        self.poll_with_retake_model_only(
            presentation,
            observations,
            now_ns,
            retirement_preflight,
            R40AggregateModelRetakeV1::Succeeds,
        )
    }

    pub fn poll_with_retake_model_only(
        self,
        presentation: &R40AggregatePresentationV1,
        observations: &[R40AggregateObservationV1],
        now_ns: u64,
        retirement_preflight: &[bool],
        model_retake: R40AggregateModelRetakeV1,
    ) -> R40AggregatePollOutcomeV1 {
        if !self.is_fully_prepared_model_only() {
            return self.terminal_model_only(R40AggregateTerminalReasonV1::PreparedStateLost, 0);
        }
        if now_ns < self.plan.started_ns {
            return self
                .terminal_model_only(R40AggregateTerminalReasonV1::InvalidObservationTime, 0);
        }
        if presentation.plan != self.plan
            || presentation.shards != self.shards
            || presentation.completion_validation_roster != self.completion_validation_roster
        {
            return self
                .terminal_model_only(R40AggregateTerminalReasonV1::PresentationSubstitution, 0);
        }
        if !presentation.currentness_closed {
            return self.terminal_model_only(R40AggregateTerminalReasonV1::CurrentnessNotClosed, 0);
        }
        if observations.len() != self.completion_validation_roster.len()
            || observations
                .iter()
                .zip(&self.completion_validation_roster)
                .any(|(observation, ticket)| observation.ticket != *ticket)
        {
            return self
                .terminal_model_only(R40AggregateTerminalReasonV1::ObservationRosterMismatch, 0);
        }

        let mut saw_pending = false;
        for (index, observation) in observations.iter().enumerate() {
            match observation.state {
                R40AggregateObservationStateV1::Ready => {}
                R40AggregateObservationStateV1::Pending => saw_pending = true,
                R40AggregateObservationStateV1::Error { terminal_token } => {
                    return self.terminal_model_only(
                        R40AggregateTerminalReasonV1::ObservationError { terminal_token },
                        index + 1,
                    );
                }
            }
        }

        if saw_pending {
            let waiting = R40AggregateWaitingV1 {
                submission: self,
                observation_count: observations.len(),
                retired_count: 0,
            };
            return if now_ns >= waiting.submission.plan.deadline_ns {
                R40AggregatePollOutcomeV1::TimedOut(waiting)
            } else {
                R40AggregatePollOutcomeV1::Pending(waiting)
            };
        }

        if retirement_preflight.len() != self.shards.len() {
            return self.terminal_model_only(
                R40AggregateTerminalReasonV1::RetirementPreflightMismatch,
                observations.len(),
            );
        }
        if let Some(index) = retirement_preflight.iter().position(|ready| !ready) {
            return self.terminal_model_only(
                R40AggregateTerminalReasonV1::RetirementPreflightFailed {
                    request_index: index as u16,
                },
                observations.len(),
            );
        }

        let Self {
            plan,
            shards,
            completion_validation_roster: _,
            mut prepared_completed_output,
        } = self;
        debug_assert!(prepared_completed_output.is_empty());
        debug_assert!(prepared_completed_output.capacity() >= shards.len());
        for shard in shards {
            prepared_completed_output.push(shard);
        }
        match model_retake {
            R40AggregateModelRetakeV1::Succeeds => {
                R40AggregatePollOutcomeV1::Completed(R40AggregateCompletedV1 {
                    retired_count: prepared_completed_output.len(),
                    observation_count: observations.len(),
                    plan,
                    shards: prepared_completed_output,
                })
            }
            R40AggregateModelRetakeV1::Fails { terminal_token } => {
                R40AggregatePollOutcomeV1::RetakeTerminal(R40AggregateRetakeTerminalV1 {
                    reason: R40AggregateTerminalReasonV1::ModelRetakeFailed { terminal_token },
                    retired_count: prepared_completed_output.len(),
                    observation_count: observations.len(),
                    plan,
                    retained_shards: prepared_completed_output,
                })
            }
        }
    }

    /// Completes retirement after a crate-internal caller has already
    /// authenticated and observed the entire completion roster and checked the
    /// entire retirement preflight, represented by an R46-private move-only
    /// witness. This deliberately does not re-observe either roster; the R40
    /// public observer remains the general checked path.
    pub(crate) fn complete_after_external_full_audit_model_only(
        self,
        witness: R46AllReadyFullAuditWitnessV1,
    ) -> R40AggregateCompletedV1 {
        debug_assert!(witness.matches_plan_model_only(&self.plan));
        let observation_count = witness.observation_count_model_only();
        debug_assert!(self.is_fully_prepared_model_only());
        debug_assert_eq!(observation_count, self.completion_validation_roster.len());
        let Self {
            plan,
            shards,
            completion_validation_roster: _,
            mut prepared_completed_output,
        } = self;
        debug_assert!(prepared_completed_output.is_empty());
        debug_assert!(prepared_completed_output.capacity() >= shards.len());
        for shard in shards {
            prepared_completed_output.push(shard);
        }
        R40AggregateCompletedV1 {
            retired_count: prepared_completed_output.len(),
            observation_count,
            plan,
            shards: prepared_completed_output,
        }
    }

    fn terminal_model_only(
        self,
        reason: R40AggregateTerminalReasonV1,
        observation_count: usize,
    ) -> R40AggregatePollOutcomeV1 {
        R40AggregatePollOutcomeV1::Terminal(R40AggregateTerminalV1 {
            submission: self,
            reason,
            observation_count,
            retired_count: 0,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R40AggregateWaitingV1 {
    submission: R40AggregateSubmissionV1,
    pub observation_count: usize,
    pub retired_count: usize,
}

impl R40AggregateWaitingV1 {
    pub fn submission_model_only(&self) -> &R40AggregateSubmissionV1 {
        &self.submission
    }

    pub fn into_submission_model_only(self) -> R40AggregateSubmissionV1 {
        self.submission
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R40AggregateTerminalV1 {
    submission: R40AggregateSubmissionV1,
    pub reason: R40AggregateTerminalReasonV1,
    pub observation_count: usize,
    pub retired_count: usize,
}

impl R40AggregateTerminalV1 {
    pub fn submission_model_only(&self) -> &R40AggregateSubmissionV1 {
        &self.submission
    }

    pub fn into_submission_model_only(self) -> R40AggregateSubmissionV1 {
        self.submission
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R40AggregateCompletedV1 {
    pub plan: R40AggregatePlanV1,
    pub shards: Vec<R40AggregateShardV1>,
    pub observation_count: usize,
    pub retired_count: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R40AggregateRetakeTerminalV1 {
    pub plan: R40AggregatePlanV1,
    retained_shards: Vec<R40AggregateShardV1>,
    pub reason: R40AggregateTerminalReasonV1,
    pub observation_count: usize,
    pub retired_count: usize,
}

impl R40AggregateRetakeTerminalV1 {
    pub fn retained_shards_model_only(&self) -> &[R40AggregateShardV1] {
        &self.retained_shards
    }

    pub fn retains_every_retired_shard_in_order_model_only(&self) -> bool {
        self.retired_count == self.retained_shards.len()
            && self.retained_shards.len() == usize::from(self.plan.request_count)
            && self
                .retained_shards
                .iter()
                .enumerate()
                .all(|(index, shard)| usize::from(shard.ticket.request_index) == index)
    }
}

impl R40AggregateCompletedV1 {
    pub fn retirement_is_complete_and_ordered_model_only(&self) -> bool {
        self.retired_count == self.shards.len()
            && self.shards.len() == usize::from(self.plan.request_count)
            && self.shards.iter().enumerate().all(|(index, shard)| {
                usize::from(shard.ticket.request_index) == index
                    && shard.ticket.submission_id == self.plan.submission_id
                    && shard.ticket.session_id == self.plan.queue_plan.session_id
                    && shard.ticket.queue_generation == self.plan.queue_generation
            })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R40AggregatePollOutcomeV1 {
    Pending(R40AggregateWaitingV1),
    TimedOut(R40AggregateWaitingV1),
    Completed(R40AggregateCompletedV1),
    Terminal(R40AggregateTerminalV1),
    RetakeTerminal(R40AggregateRetakeTerminalV1),
}

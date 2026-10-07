//! Executable finite R46 model for an optimized blocking wait over one R40
//! gfx942 striped-SDMA aggregate.
//!
//! Binding consumes the exact R40 submission and its immutable presentation,
//! partitions the ordered request roster by active striped queue, and validates
//! one authenticated tail-fence identity per active shard. The tail-fence
//! ordering rule is a caller-supplied contract: on one admitted gfx942 SDMA
//! engine, a system-scope tail completion becomes ready only after every
//! preceding copy or fence occurrence on that shard is complete and visible.
//! This model checks the contract fields but does not establish their native
//! truth.
//!
//! A nonfinal round observes exactly the authenticated shard tails. Pending
//! retains the complete move-only wrapper without auditing the request roster.
//! Tail error, invalid currentness, and identity substitution are terminal. If
//! every tail is ready, or the shared absolute deadline has been reached, the
//! model performs one exact full ordered-roster audit. Timeout requires a
//! Pending entry in that final audit. A ready tail with a Pending prefix is a
//! named fail-closed contract violation. Only an all-ready final audit followed
//! by a complete retirement preflight mints an R46-private move-only witness
//! consumed by a crate-private R40 retirement move. That move does not
//! re-observe the already-audited roster. The public R40 general observer
//! remains unchanged.
//!
//! All vectors and output capacity exist before the first wait round. Abstract
//! observation accounting for admitted exact rounds is
//! `rounds * active_shards + final_audit_entries`. Malformed identity prefixes
//! fail before committing an abstract observation count. Identity comparisons
//! and the separate final retirement moves are not observation events. This
//! module performs no I/O and proves no Rust refinement, allocation or panic
//! behavior, CPU work count, KFD/HSA/HIP behavior, native fence semantics,
//! clock truth, hardware property, progress, parity, or performance gain.

use alloc::vec::Vec;

#[cfg(test)]
use crate::R40AggregateTicketV1;
use crate::{
    R40AggregateCompletedV1, R40AggregateObservationStateV1, R40AggregateObservationV1,
    R40AggregatePlanV1, R40AggregatePresentationV1, R40AggregateSubmissionV1,
    R40Gfx942SdmaQueueRoleV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R46TailFenceContractV1 {
    pub admitted_gfx942_sdma_engine: bool,
    pub system_scope_completion_fence: bool,
    /// Contracted native fact: reading the named signal occurrence observes
    /// completion of this tail's exact `fence_occurrence` and no other fence.
    pub signal_read_is_bound_to_named_fence: bool,
    pub completion_implies_preceding_visible: bool,
}

impl R46TailFenceContractV1 {
    pub const fn contracted_model_only() -> Self {
        Self {
            admitted_gfx942_sdma_engine: true,
            system_scope_completion_fence: true,
            signal_read_is_bound_to_named_fence: true,
            completion_implies_preceding_visible: true,
        }
    }

    pub const fn is_exact_model_only(self) -> bool {
        self.admitted_gfx942_sdma_engine
            && self.system_scope_completion_fence
            && self.signal_read_is_bound_to_named_fence
            && self.completion_implies_preceding_visible
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R46AuthenticatedTailFenceV1 {
    pub session_id: u64,
    pub submission_id: u64,
    pub engine_index: u8,
    pub queue_slot: u8,
    pub queue_id: u32,
    pub queue_generation: u64,
    pub last_request_index: u16,
    pub fence_occurrence: u64,
    pub signal_slot: u16,
    pub signal_generation: u64,
    pub contract: R46TailFenceContractV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R46ActiveShardV1 {
    pub engine_index: u8,
    pub queue_slot: u8,
    pub queue_id: u32,
    pub queue_generation: u64,
    pub request_indices: Vec<u16>,
    pub tail: R46AuthenticatedTailFenceV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R46TailCurrentnessV1 {
    pub session_id: u64,
    pub submission_id: u64,
    pub queue_generation: u64,
    pub scope_closed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R46TailObservationStateV1 {
    Ready,
    Pending,
    Error { terminal_token: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R46TailObservationV1 {
    pub tail: R46AuthenticatedTailFenceV1,
    pub state: R46TailObservationStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R46FinalAuditEntryV1 {
    pub observation: R40AggregateObservationV1,
    pub retirement_ready: bool,
}

/// Move-only internal authority to retire after one exact all-ready audit.
/// Only this module can mint the witness; the R40 owner can only consume it.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct R46AllReadyFullAuditWitnessV1 {
    session_id: u64,
    submission_id: u64,
    queue_generation: u64,
    observation_count: usize,
}

impl R46AllReadyFullAuditWitnessV1 {
    pub(crate) fn observation_count_model_only(&self) -> usize {
        self.observation_count
    }

    pub(crate) fn matches_plan_model_only(&self, plan: &R40AggregatePlanV1) -> bool {
        self.session_id == plan.queue_plan.session_id
            && self.submission_id == plan.submission_id
            && self.queue_generation == plan.queue_generation
            && self.observation_count == usize::from(plan.request_count)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R46TailWaitBindErrorV1 {
    R40SubmissionNotPrepared,
    R40PresentationSubstitution,
    CurrentnessNotClosedAtBinding,
    TailCountMismatch,
    TailSubstitution { active_shard_index: u8 },
    TailFenceOccurrenceNotUnique { active_shard_index: u8 },
    TailSignalNotUnique { active_shard_index: u8 },
    TailContractRejected { active_shard_index: u8 },
}

#[derive(Debug, Eq, PartialEq)]
pub struct R46UnboundTailWaitV1 {
    submission: R40AggregateSubmissionV1,
    presentation: R40AggregatePresentationV1,
    tails: Vec<R46AuthenticatedTailFenceV1>,
}

impl R46UnboundTailWaitV1 {
    pub fn submission_model_only(&self) -> &R40AggregateSubmissionV1 {
        &self.submission
    }

    pub fn tails_model_only(&self) -> &[R46AuthenticatedTailFenceV1] {
        &self.tails
    }

    pub fn into_parts_model_only(
        self,
    ) -> (
        R40AggregateSubmissionV1,
        R40AggregatePresentationV1,
        Vec<R46AuthenticatedTailFenceV1>,
    ) {
        (self.submission, self.presentation, self.tails)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R46TailWaitTerminalReasonV1 {
    InvalidObservationTime,
    CurrentnessFailure,
    TailObservationRosterMismatch,
    TailObservationError {
        active_shard_index: u8,
        terminal_token: u64,
    },
    ObservationCounterOverflow,
    FinalAuditRosterMismatch,
    FinalAuditError {
        request_index: u16,
        terminal_token: u64,
    },
    TailReadyWithPendingPrefix {
        request_index: u16,
    },
    RetirementPreflightFailed {
        request_index: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R46TailWaitWorkV1 {
    pub tail_rounds: u64,
    pub active_shards: u8,
    pub tail_observations: u64,
    pub final_audit_observations: u16,
}

impl R46TailWaitWorkV1 {
    pub fn exact_complexity_model_only(self) -> bool {
        self.tail_rounds.checked_mul(u64::from(self.active_shards)) == Some(self.tail_observations)
    }

    pub fn total_observations_model_only(self) -> Option<u64> {
        self.tail_observations
            .checked_add(u64::from(self.final_audit_observations))
    }
}

/// Move-only owner of the exact R40 aggregate and its prebound tail roster.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R46Gfx942StripedSdmaTailWaitV1;
/// let owner: R46Gfx942StripedSdmaTailWaitV1 = todo!();
/// let duplicate = owner.clone();
/// # let _ = duplicate;
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R46Gfx942StripedSdmaTailWaitV1 {
    submission: R40AggregateSubmissionV1,
    presentation: R40AggregatePresentationV1,
    active_shards: Vec<R46ActiveShardV1>,
    tail_rounds: u64,
    tail_observations: u64,
    final_audit_observations: u16,
    post_bind_allocation_count: u8,
}

#[derive(Debug, Eq, PartialEq)]
pub enum R46TailWaitBindOutcomeV1 {
    Bound(R46Gfx942StripedSdmaTailWaitV1),
    Rejected {
        reason: R46TailWaitBindErrorV1,
        unbound: R46UnboundTailWaitV1,
    },
}

impl R46Gfx942StripedSdmaTailWaitV1 {
    pub fn bind_model_only(
        submission: R40AggregateSubmissionV1,
        presentation: R40AggregatePresentationV1,
        tails: Vec<R46AuthenticatedTailFenceV1>,
    ) -> R46TailWaitBindOutcomeV1 {
        let reject = |reason, submission, presentation, tails| R46TailWaitBindOutcomeV1::Rejected {
            reason,
            unbound: R46UnboundTailWaitV1 {
                submission,
                presentation,
                tails,
            },
        };
        if !submission.is_fully_prepared_model_only() {
            return reject(
                R46TailWaitBindErrorV1::R40SubmissionNotPrepared,
                submission,
                presentation,
                tails,
            );
        }
        if submission.plan_model_only() != &presentation.plan
            || submission.shards_model_only() != presentation.shards.as_slice()
            || submission.completion_validation_roster_model_only()
                != presentation.completion_validation_roster.as_slice()
        {
            return reject(
                R46TailWaitBindErrorV1::R40PresentationSubstitution,
                submission,
                presentation,
                tails,
            );
        }
        if !presentation.currentness_closed {
            return reject(
                R46TailWaitBindErrorV1::CurrentnessNotClosedAtBinding,
                submission,
                presentation,
                tails,
            );
        }

        let plan = submission.plan_model_only().clone();
        let mut active_shards = Vec::with_capacity(
            usize::from(plan.queue_plan.striped_queue_count).min(usize::from(plan.request_count)),
        );
        for queue_slot in 0..plan.queue_plan.striped_queue_count {
            let mut request_indices = Vec::new();
            for shard in submission.shards_model_only() {
                if shard.ticket.queue_slot == queue_slot {
                    request_indices.push(shard.ticket.request_index);
                }
            }
            if request_indices.is_empty() {
                continue;
            }
            let queue = plan
                .queue_plan
                .striped_queue_model_only(queue_slot)
                .expect("an exact R40 plan has every striped queue");
            debug_assert_eq!(queue.role, R40Gfx942SdmaQueueRoleV1::Striped);
            active_shards.push(R46ActiveShardV1 {
                engine_index: queue.engine_index,
                queue_slot,
                queue_id: queue.queue_id,
                queue_generation: plan.queue_generation,
                request_indices,
                tail: R46AuthenticatedTailFenceV1 {
                    session_id: 0,
                    submission_id: 0,
                    engine_index: 0,
                    queue_slot: 0,
                    queue_id: 0,
                    queue_generation: 0,
                    last_request_index: 0,
                    fence_occurrence: 0,
                    signal_slot: 0,
                    signal_generation: 0,
                    contract: R46TailFenceContractV1 {
                        admitted_gfx942_sdma_engine: false,
                        system_scope_completion_fence: false,
                        signal_read_is_bound_to_named_fence: false,
                        completion_implies_preceding_visible: false,
                    },
                },
            });
        }
        if tails.len() != active_shards.len() {
            return reject(
                R46TailWaitBindErrorV1::TailCountMismatch,
                submission,
                presentation,
                tails,
            );
        }

        for (index, (shard, tail)) in active_shards.iter_mut().zip(&tails).enumerate() {
            let expected_last = *shard
                .request_indices
                .last()
                .expect("active shard has at least one request");
            if tail.session_id != plan.queue_plan.session_id
                || tail.submission_id != plan.submission_id
                || tail.engine_index != shard.engine_index
                || tail.queue_slot != shard.queue_slot
                || tail.queue_id != shard.queue_id
                || tail.queue_generation != shard.queue_generation
                || tail.last_request_index != expected_last
                || tail.fence_occurrence == 0
                || tail.signal_generation == 0
            {
                return reject(
                    R46TailWaitBindErrorV1::TailSubstitution {
                        active_shard_index: index as u8,
                    },
                    submission,
                    presentation,
                    tails,
                );
            }
            if tails[..index]
                .iter()
                .any(|prior| prior.fence_occurrence == tail.fence_occurrence)
            {
                return reject(
                    R46TailWaitBindErrorV1::TailFenceOccurrenceNotUnique {
                        active_shard_index: index as u8,
                    },
                    submission,
                    presentation,
                    tails,
                );
            }
            if tails[..index].iter().any(|prior| {
                prior.signal_slot == tail.signal_slot
                    && prior.signal_generation == tail.signal_generation
            }) {
                return reject(
                    R46TailWaitBindErrorV1::TailSignalNotUnique {
                        active_shard_index: index as u8,
                    },
                    submission,
                    presentation,
                    tails,
                );
            }
            if !tail.contract.is_exact_model_only() {
                return reject(
                    R46TailWaitBindErrorV1::TailContractRejected {
                        active_shard_index: index as u8,
                    },
                    submission,
                    presentation,
                    tails,
                );
            }
            shard.tail = *tail;
        }

        R46TailWaitBindOutcomeV1::Bound(Self {
            submission,
            presentation,
            active_shards,
            tail_rounds: 0,
            tail_observations: 0,
            final_audit_observations: 0,
            post_bind_allocation_count: 0,
        })
    }

    pub fn submission_model_only(&self) -> &R40AggregateSubmissionV1 {
        &self.submission
    }

    pub fn active_shards_model_only(&self) -> &[R46ActiveShardV1] {
        &self.active_shards
    }

    pub fn work_model_only(&self) -> R46TailWaitWorkV1 {
        R46TailWaitWorkV1 {
            tail_rounds: self.tail_rounds,
            active_shards: self.active_shards.len() as u8,
            tail_observations: self.tail_observations,
            final_audit_observations: self.final_audit_observations,
        }
    }

    pub fn post_bind_allocation_count_model_only(&self) -> u8 {
        self.post_bind_allocation_count
    }

    pub fn is_exact_model_only(&self) -> bool {
        let plan = self.submission.plan_model_only();
        self.submission.is_fully_prepared_model_only()
            && self.presentation.plan == *plan
            && self.presentation.shards.as_slice() == self.submission.shards_model_only()
            && self.presentation.completion_validation_roster.as_slice()
                == self.submission.completion_validation_roster_model_only()
            && self.presentation.currentness_closed
            && self.active_shards.len()
                == usize::from(
                    plan.request_count
                        .min(u16::from(plan.queue_plan.striped_queue_count)),
                )
            && self.active_shards.iter().enumerate().all(|(index, shard)| {
                usize::from(shard.queue_slot) == index
                    && plan
                        .queue_plan
                        .striped_queue_model_only(shard.queue_slot)
                        .is_some_and(|queue| {
                            queue.engine_index == shard.engine_index
                                && queue.queue_id == shard.queue_id
                        })
                    && !shard.request_indices.is_empty()
                    && shard
                        .request_indices
                        .iter()
                        .enumerate()
                        .all(|(order, request_index)| {
                            usize::from(*request_index)
                                == index + order * usize::from(plan.queue_plan.striped_queue_count)
                        })
                    && shard.tail.session_id == plan.queue_plan.session_id
                    && shard.tail.submission_id == plan.submission_id
                    && shard.tail.queue_slot == shard.queue_slot
                    && shard.tail.queue_id == shard.queue_id
                    && shard.tail.queue_generation == shard.queue_generation
                    && shard.tail.engine_index == shard.engine_index
                    && shard.tail.fence_occurrence != 0
                    && shard.tail.signal_generation != 0
                    && shard.tail.contract.is_exact_model_only()
                    && shard
                        .request_indices
                        .last()
                        .is_some_and(|last| *last == shard.tail.last_request_index)
                    && self.active_shards[..index].iter().all(|prior| {
                        prior.tail.fence_occurrence != shard.tail.fence_occurrence
                            && (prior.tail.signal_slot != shard.tail.signal_slot
                                || prior.tail.signal_generation != shard.tail.signal_generation)
                    })
            })
            && self.work_model_only().exact_complexity_model_only()
            && (self.final_audit_observations == 0
                || self.final_audit_observations == plan.request_count)
            && self.post_bind_allocation_count == 0
    }

    pub fn is_resumable_model_only(&self) -> bool {
        self.is_exact_model_only() && self.final_audit_observations == 0
    }

    pub fn wait_round_model_only(
        mut self,
        currentness: R46TailCurrentnessV1,
        tail_observations: &[R46TailObservationV1],
        final_audit: &[R46FinalAuditEntryV1],
        now_ns: u64,
    ) -> R46TailWaitOutcomeV1 {
        if now_ns < self.submission.plan_model_only().started_ns {
            return self.terminal_model_only(R46TailWaitTerminalReasonV1::InvalidObservationTime);
        }
        if !self.currentness_is_exact_model_only(currentness) {
            return self.terminal_model_only(R46TailWaitTerminalReasonV1::CurrentnessFailure);
        }
        if tail_observations.len() != self.active_shards.len() {
            return self
                .terminal_model_only(R46TailWaitTerminalReasonV1::TailObservationRosterMismatch);
        }

        let Some(next_rounds) = self.tail_rounds.checked_add(1) else {
            return self
                .terminal_model_only(R46TailWaitTerminalReasonV1::ObservationCounterOverflow);
        };
        let Some(next_tail_observations) = self
            .tail_observations
            .checked_add(self.active_shards.len() as u64)
        else {
            return self
                .terminal_model_only(R46TailWaitTerminalReasonV1::ObservationCounterOverflow);
        };
        let mut saw_pending_tail = false;
        let mut first_tail_error = None;
        for (index, (observation, shard)) in tail_observations
            .iter()
            .zip(&self.active_shards)
            .enumerate()
        {
            if observation.tail != shard.tail {
                return self.terminal_model_only(
                    R46TailWaitTerminalReasonV1::TailObservationRosterMismatch,
                );
            }
            match observation.state {
                R46TailObservationStateV1::Ready => {}
                R46TailObservationStateV1::Pending => saw_pending_tail = true,
                R46TailObservationStateV1::Error { terminal_token } => {
                    if first_tail_error.is_none() {
                        first_tail_error = Some((index as u8, terminal_token));
                    }
                }
            }
        }
        self.tail_rounds = next_rounds;
        self.tail_observations = next_tail_observations;
        if let Some((active_shard_index, terminal_token)) = first_tail_error {
            return self.terminal_model_only(R46TailWaitTerminalReasonV1::TailObservationError {
                active_shard_index,
                terminal_token,
            });
        }

        let deadline_reached = now_ns >= self.submission.plan_model_only().deadline_ns;
        if saw_pending_tail && !deadline_reached {
            return R46TailWaitOutcomeV1::Pending(R46TailWaitingV1 { owner: self });
        }

        let request_count = self
            .submission
            .completion_validation_roster_model_only()
            .len();
        if final_audit.len() != request_count {
            return self.terminal_model_only(R46TailWaitTerminalReasonV1::FinalAuditRosterMismatch);
        }
        let mut first_pending = None;
        let mut first_error = None;
        let mut first_failed_preflight = None;
        for (index, (entry, ticket)) in final_audit
            .iter()
            .zip(self.submission.completion_validation_roster_model_only())
            .enumerate()
        {
            let observation = entry.observation;
            if observation.ticket != *ticket {
                return self
                    .terminal_model_only(R46TailWaitTerminalReasonV1::FinalAuditRosterMismatch);
            }
            match observation.state {
                R40AggregateObservationStateV1::Ready => {}
                R40AggregateObservationStateV1::Pending => {
                    if first_pending.is_none() {
                        first_pending = Some(observation.ticket.request_index);
                    }
                }
                R40AggregateObservationStateV1::Error { terminal_token } => {
                    if first_error.is_none() {
                        first_error = Some((observation.ticket.request_index, terminal_token));
                    }
                }
            }
            if !entry.retirement_ready && first_failed_preflight.is_none() {
                first_failed_preflight = Some(index as u16);
            }
        }
        self.final_audit_observations = request_count as u16;
        if let Some((request_index, terminal_token)) = first_error {
            return self.terminal_model_only(R46TailWaitTerminalReasonV1::FinalAuditError {
                request_index,
                terminal_token,
            });
        }
        if let Some(request_index) = first_pending {
            if !saw_pending_tail {
                return self.terminal_model_only(
                    R46TailWaitTerminalReasonV1::TailReadyWithPendingPrefix { request_index },
                );
            }
            return R46TailWaitOutcomeV1::TimedOut(R46TailTimedOutV1 { owner: self });
        }
        if let Some(request_index) = first_failed_preflight {
            return self.terminal_model_only(
                R46TailWaitTerminalReasonV1::RetirementPreflightFailed { request_index },
            );
        }

        let plan = self.submission.plan_model_only();
        let witness = R46AllReadyFullAuditWitnessV1 {
            session_id: plan.queue_plan.session_id,
            submission_id: plan.submission_id,
            queue_generation: plan.queue_generation,
            observation_count: request_count,
        };
        self.complete_via_r40_model_only(witness)
    }

    fn currentness_is_exact_model_only(&self, currentness: R46TailCurrentnessV1) -> bool {
        let plan = self.submission.plan_model_only();
        currentness.session_id == plan.queue_plan.session_id
            && currentness.submission_id == plan.submission_id
            && currentness.queue_generation == plan.queue_generation
            && currentness.scope_closed
    }

    fn complete_via_r40_model_only(
        self,
        witness: R46AllReadyFullAuditWitnessV1,
    ) -> R46TailWaitOutcomeV1 {
        let Self {
            submission,
            presentation: _,
            active_shards,
            tail_rounds,
            tail_observations,
            final_audit_observations,
            post_bind_allocation_count,
        } = self;
        let active_shard_count = active_shards.len() as u8;
        let work = R46TailWaitWorkV1 {
            tail_rounds,
            active_shards: active_shard_count,
            tail_observations,
            final_audit_observations,
        };
        let completed = submission.complete_after_external_full_audit_model_only(witness);
        R46TailWaitOutcomeV1::Completed(R46TailCompletedV1 {
            completed,
            work,
            post_bind_allocation_count,
        })
    }

    fn terminal_model_only(self, reason: R46TailWaitTerminalReasonV1) -> R46TailWaitOutcomeV1 {
        let work = self.work_model_only();
        R46TailWaitOutcomeV1::Terminal(R46TailTerminalV1 {
            owner: self,
            reason,
            work,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R46TailWaitingV1 {
    owner: R46Gfx942StripedSdmaTailWaitV1,
}

impl R46TailWaitingV1 {
    pub fn owner_model_only(&self) -> &R46Gfx942StripedSdmaTailWaitV1 {
        &self.owner
    }

    pub fn into_owner_model_only(self) -> R46Gfx942StripedSdmaTailWaitV1 {
        self.owner
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R46TailTimedOutV1 {
    owner: R46Gfx942StripedSdmaTailWaitV1,
}

impl R46TailTimedOutV1 {
    pub fn owner_model_only(&self) -> &R46Gfx942StripedSdmaTailWaitV1 {
        &self.owner
    }

    /// Timeout is a final custody receipt. It deliberately has no consuming
    /// resume operation; a future wait epoch requires a separately modeled
    /// transition rather than silently repeating the one-final-audit contract.
    pub fn is_exact_terminal_custody_model_only(&self) -> bool {
        self.owner.is_exact_model_only()
            && self.owner.work_model_only().final_audit_observations
                == self
                    .owner
                    .submission_model_only()
                    .plan_model_only()
                    .request_count
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R46TailTerminalV1 {
    owner: R46Gfx942StripedSdmaTailWaitV1,
    pub reason: R46TailWaitTerminalReasonV1,
    pub work: R46TailWaitWorkV1,
}

impl R46TailTerminalV1 {
    pub fn owner_model_only(&self) -> &R46Gfx942StripedSdmaTailWaitV1 {
        &self.owner
    }

    pub fn is_exact_terminal_custody_model_only(&self) -> bool {
        self.owner.is_exact_model_only()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R46TailCompletedV1 {
    pub completed: R40AggregateCompletedV1,
    pub work: R46TailWaitWorkV1,
    pub post_bind_allocation_count: u8,
}

impl R46TailCompletedV1 {
    pub fn is_exact_model_only(&self) -> bool {
        self.completed
            .retirement_is_complete_and_ordered_model_only()
            && self.work.exact_complexity_model_only()
            && self.work.final_audit_observations == self.completed.plan.request_count
            && self.post_bind_allocation_count == 0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R46TailWaitOutcomeV1 {
    Pending(R46TailWaitingV1),
    TimedOut(R46TailTimedOutV1),
    Completed(R46TailCompletedV1),
    Terminal(R46TailTerminalV1),
}

#[cfg(test)]
pub(crate) fn r46_tail_observations_model_only(
    owner: &R46Gfx942StripedSdmaTailWaitV1,
    state: R46TailObservationStateV1,
) -> Vec<R46TailObservationV1> {
    owner
        .active_shards_model_only()
        .iter()
        .map(|shard| R46TailObservationV1 {
            tail: shard.tail,
            state,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn r46_final_audit_model_only(
    owner: &R46Gfx942StripedSdmaTailWaitV1,
    state: R40AggregateObservationStateV1,
) -> Vec<R46FinalAuditEntryV1> {
    owner
        .submission_model_only()
        .completion_validation_roster_model_only()
        .iter()
        .map(|ticket: &R40AggregateTicketV1| R46FinalAuditEntryV1 {
            observation: R40AggregateObservationV1 {
                ticket: *ticket,
                state,
            },
            retirement_ready: true,
        })
        .collect()
}

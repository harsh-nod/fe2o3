//! Executable finite R48 model for the retryable gfx942 striped-SDMA tail wait.
//!
//! R48 supersedes R46 only where the production-shaped domain differs: it
//! admits rotating first-queue cursors, standalone sixteen-queue submissions,
//! retryable timeout epochs, early observation-error prefixes, panic-guard
//! custody, per-physical-queue tail ordering, the full 64-slot ring domain, and
//! post-retirement model-retake failure. Packet publication order, signal
//! binding, currentness, tail ordering, time, and panic interception are explicit
//! mathematical premises. This module performs no I/O and establishes
//! no production-Rust, allocator, KFD/HSA/HIP, firmware, coherence, hardware,
//! progress, parity, or performance claim.

use alloc::vec::Vec;
use core::num::NonZeroU64;

pub const R48_MAX_STRIPED_QUEUES_V1: usize = 16;
pub const R48_MAX_COMBINED_STRIPED_QUEUES_V1: usize = 14;
pub const R48_REQUESTS_PER_SHARD_V1: usize = 63;
pub const R48_RING_SLOT_COUNT_V1: usize = 64;
pub const R48_MAX_REQUESTS_V1: usize = 1008;
pub const R48_SYSTEM_SNOOP_FENCE_HEADER_V1: u32 = 0x0053_0005;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R48StripedQueueProfileV1 {
    Combined,
    Standalone,
}

impl R48StripedQueueProfileV1 {
    pub const fn admits_queue_count_model_only(self, queue_count: usize) -> bool {
        queue_count >= 2
            && queue_count.is_multiple_of(2)
            && match self {
                Self::Combined => queue_count <= R48_MAX_COMBINED_STRIPED_QUEUES_V1,
                Self::Standalone => queue_count <= R48_MAX_STRIPED_QUEUES_V1,
            }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R48SubmissionOccurrenceV1 {
    owner_occurrence: u64,
    session_occurrence: u64,
    epoch: u64,
    minted_owner_occurrence: u64,
    minted_session_occurrence: u64,
    minted_epoch: u64,
}

impl R48SubmissionOccurrenceV1 {
    pub const fn epoch_model_only(&self) -> u64 {
        self.epoch
    }

    pub fn substitute_epoch_model_only(mut self, epoch: u64) -> Self {
        self.epoch = epoch;
        self
    }

    pub fn substitute_owner_model_only(mut self, owner_occurrence: u64) -> Self {
        self.owner_occurrence = owner_occurrence;
        self
    }

    fn is_authentic_model_only(&self) -> bool {
        self.owner_occurrence != 0
            && self.session_occurrence != 0
            && self.epoch != 0
            && self.minted_owner_occurrence == self.owner_occurrence
            && self.minted_session_occurrence == self.session_occurrence
            && self.minted_epoch == self.epoch
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R48SubmissionEpochOwnerV1 {
    owner_occurrence: u64,
    session_occurrence: u64,
    next_epoch: u64,
}

impl R48SubmissionEpochOwnerV1 {
    pub fn new_model_only(
        owner_occurrence: u64,
        session_occurrence: u64,
    ) -> Result<Self, R48TailWaitErrorV1> {
        if owner_occurrence == 0 || session_occurrence == 0 {
            return Err(R48TailWaitErrorV1::InvalidSubmissionIdentity);
        }
        Ok(Self {
            owner_occurrence,
            session_occurrence,
            next_epoch: 1,
        })
    }

    pub const fn next_epoch_model_only(&self) -> u64 {
        self.next_epoch
    }

    pub fn mint_model_only(&mut self) -> Result<R48SubmissionOccurrenceV1, R48TailWaitErrorV1> {
        let epoch = self.next_epoch;
        let next_epoch = epoch
            .checked_add(1)
            .ok_or(R48TailWaitErrorV1::SubmissionEpochExhausted)?;
        self.next_epoch = next_epoch;
        Ok(R48SubmissionOccurrenceV1 {
            owner_occurrence: self.owner_occurrence,
            session_occurrence: self.session_occurrence,
            epoch,
            minted_owner_occurrence: self.owner_occurrence,
            minted_session_occurrence: self.session_occurrence,
            minted_epoch: epoch,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R48SignalIdentityV1 {
    pub mapping_identity: u64,
    pub slot: u16,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R48PhysicalEngineLabelV1 {
    Engine0,
    Engine1,
}

impl R48PhysicalEngineLabelV1 {
    pub const fn for_queue_ordinal_model_only(queue_ordinal: usize) -> Self {
        if queue_ordinal.is_multiple_of(2) {
            Self::Engine0
        } else {
            Self::Engine1
        }
    }
}

impl R48SignalIdentityV1 {
    pub const fn is_exact_model_only(self) -> bool {
        self.mapping_identity != 0 && self.generation != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R48StripedTicketV1 {
    pub owner_occurrence: u64,
    pub session_occurrence: u64,
    pub submission_epoch: u64,
    pub request_index: u16,
    pub queue_ordinal: u8,
    pub queue_id: u32,
    pub engine: R48PhysicalEngineLabelV1,
    pub queue_generation: u64,
    pub ring_slot: u16,
    pub signal: R48SignalIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R48PacketPublicationEvidenceV1 {
    pub ticket: R48StripedTicketV1,
    pub packet_occurrence: u64,
    pub fence_header: u32,
    pub packet_signal: R48SignalIdentityV1,
    pub complete_packet_written: bool,
    pub complete_packet_before_write_pointer_release: bool,
    pub record_retained_before_write_pointer_release: bool,
    pub write_pointer_published_release: bool,
    pub doorbell_published_release: bool,
    pub write_pointer_release_before_doorbell_release: bool,
    pub admitted_gfx942_engine: bool,
    pub system_scope: bool,
    pub snoop: bool,
    pub signal_read_names_fence: bool,
    pub completion_implies_preceding_visible: bool,
}

impl R48PacketPublicationEvidenceV1 {
    pub fn is_exact_for_model_only(self, ticket: R48StripedTicketV1) -> bool {
        self.ticket == ticket
            && self.packet_occurrence != 0
            && self.fence_header == R48_SYSTEM_SNOOP_FENCE_HEADER_V1
            && self.packet_signal == ticket.signal
            && ticket.signal.is_exact_model_only()
            && self.complete_packet_written
            && self.complete_packet_before_write_pointer_release
            && self.record_retained_before_write_pointer_release
            && self.write_pointer_published_release
            && self.doorbell_published_release
            && self.write_pointer_release_before_doorbell_release
            && self.admitted_gfx942_engine
            && self.system_scope
            && self.snoop
            && self.signal_read_names_fence
            && self.completion_implies_preceding_visible
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R48PublishedRequestV1 {
    pub ticket: R48StripedTicketV1,
    pub publication: R48PacketPublicationEvidenceV1,
    pub payload_identity: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R48BoundTailV1 {
    pub normalized_queue_slot: u8,
    pub ticket: R48StripedTicketV1,
    pub publication: R48PacketPublicationEvidenceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R48StripedPlanV1 {
    pub profile: R48StripedQueueProfileV1,
    pub owner_occurrence: u64,
    pub session_occurrence: u64,
    pub submission_epoch: u64,
    pub queue_generation: u64,
    pub first_queue: u8,
    pub queue_ids: Vec<u32>,
    pub request_count: u16,
}

impl R48StripedPlanV1 {
    pub const fn queue_count_model_only(&self) -> usize {
        self.queue_ids.len()
    }

    pub fn queue_for_request_model_only(&self, request_index: usize) -> Option<usize> {
        let queue_count = self.queue_ids.len();
        if queue_count == 0
            || usize::from(self.first_queue) >= queue_count
            || request_index >= usize::from(self.request_count)
        {
            return None;
        }
        usize::from(self.first_queue)
            .checked_add(request_index)
            .map(|assigned| assigned % queue_count)
    }

    pub fn normalized_slot_model_only(&self, queue_ordinal: usize) -> Option<usize> {
        let queue_count = self.queue_ids.len();
        let first_queue = usize::from(self.first_queue);
        if queue_count == 0 || first_queue >= queue_count || queue_ordinal >= queue_count {
            return None;
        }
        if queue_ordinal >= first_queue {
            Some(queue_ordinal - first_queue)
        } else {
            queue_ordinal.checked_add(queue_count - first_queue)
        }
    }

    pub const fn active_shard_count_model_only(&self) -> usize {
        let requests = self.request_count as usize;
        let queues = self.queue_ids.len();
        if requests < queues { requests } else { queues }
    }

    pub fn is_exact_model_only(&self) -> bool {
        let queue_count = self.queue_ids.len();
        self.profile.admits_queue_count_model_only(queue_count)
            && self.owner_occurrence != 0
            && self.session_occurrence != 0
            && self.submission_epoch != 0
            && self.queue_generation != 0
            && usize::from(self.first_queue) < queue_count
            && self.request_count != 0
            && usize::from(self.request_count)
                <= queue_count
                    .saturating_mul(R48_REQUESTS_PER_SHARD_V1)
                    .min(R48_MAX_REQUESTS_V1)
            && self.queue_ids.iter().all(|queue_id| *queue_id != 0)
            && self
                .queue_ids
                .iter()
                .enumerate()
                .all(|(index, queue_id)| !self.queue_ids[..index].contains(queue_id))
    }
}

/// Move-only owner of an exact published submission and its evidence roster.
#[derive(Debug, Eq, PartialEq)]
pub struct R48PublishedStripedSubmissionV1 {
    plan: R48StripedPlanV1,
    requests: Vec<R48PublishedRequestV1>,
    tails: Vec<R48BoundTailV1>,
}

impl R48PublishedStripedSubmissionV1 {
    pub fn seal_model_only(
        occurrence: R48SubmissionOccurrenceV1,
        profile: R48StripedQueueProfileV1,
        queue_generation: u64,
        first_queue: usize,
        queue_ids: Vec<u32>,
        requests: Vec<R48PublishedRequestV1>,
    ) -> Result<Self, (R48TailWaitErrorV1, Vec<R48PublishedRequestV1>)> {
        let occurrence_exact = occurrence.is_authentic_model_only();
        let request_count = match u16::try_from(requests.len()) {
            Ok(count) => count,
            Err(_) => return Err((R48TailWaitErrorV1::RequestCapacity, requests)),
        };
        let Ok(first_queue) = u8::try_from(first_queue) else {
            return Err((R48TailWaitErrorV1::InvalidCursor, requests));
        };
        let plan = R48StripedPlanV1 {
            profile,
            owner_occurrence: occurrence.owner_occurrence,
            session_occurrence: occurrence.session_occurrence,
            submission_epoch: occurrence.epoch,
            queue_generation,
            first_queue,
            queue_ids,
            request_count,
        };
        if !occurrence_exact || !plan.is_exact_model_only() {
            return Err((R48TailWaitErrorV1::InvalidSubmissionIdentity, requests));
        }
        for (index, request) in requests.iter().enumerate() {
            let Some(queue_ordinal) = plan.queue_for_request_model_only(index) else {
                return Err((R48TailWaitErrorV1::InvalidProfile, requests));
            };
            let expected = R48StripedTicketV1 {
                owner_occurrence: plan.owner_occurrence,
                session_occurrence: plan.session_occurrence,
                submission_epoch: plan.submission_epoch,
                request_index: index as u16,
                queue_ordinal: queue_ordinal as u8,
                queue_id: plan.queue_ids[queue_ordinal],
                engine: R48PhysicalEngineLabelV1::for_queue_ordinal_model_only(queue_ordinal),
                queue_generation: plan.queue_generation,
                ring_slot: request.ticket.ring_slot,
                signal: request.ticket.signal,
            };
            if request.payload_identity == 0
                || request.ticket != expected
                || !request.publication.is_exact_for_model_only(expected)
                || usize::from(request.ticket.ring_slot) >= R48_RING_SLOT_COUNT_V1
                || requests[..index].iter().any(|prior| {
                    (prior.ticket.queue_ordinal == request.ticket.queue_ordinal
                        && (prior.publication.packet_occurrence
                            == request.publication.packet_occurrence
                            || prior.ticket.ring_slot == request.ticket.ring_slot))
                        || prior.ticket.signal == request.ticket.signal
                })
            {
                return Err((R48TailWaitErrorV1::PublicationEvidence, requests));
            }
        }
        let mut tails = Vec::with_capacity(plan.active_shard_count_model_only());
        for normalized_slot in 0..plan.active_shard_count_model_only() {
            let last_request = normalized_slot
                + ((usize::from(plan.request_count) - 1 - normalized_slot)
                    / plan.queue_count_model_only())
                    * plan.queue_count_model_only();
            let request = requests[last_request];
            tails.push(R48BoundTailV1 {
                normalized_queue_slot: normalized_slot as u8,
                ticket: request.ticket,
                publication: request.publication,
            });
        }
        let submission = Self {
            plan,
            requests,
            tails,
        };
        if !submission.is_exact_model_only() {
            let Self { requests, .. } = submission;
            return Err((R48TailWaitErrorV1::PublicationEvidence, requests));
        }
        Ok(submission)
    }

    pub const fn plan_model_only(&self) -> &R48StripedPlanV1 {
        &self.plan
    }

    pub fn requests_model_only(&self) -> &[R48PublishedRequestV1] {
        &self.requests
    }

    pub fn tails_model_only(&self) -> &[R48BoundTailV1] {
        &self.tails
    }

    pub fn is_exact_model_only(&self) -> bool {
        self.plan.is_exact_model_only()
            && self.requests.len() == usize::from(self.plan.request_count)
            && self.tails.len() == self.plan.active_shard_count_model_only()
            && self.requests.iter().enumerate().all(|(index, request)| {
                self.plan
                    .queue_for_request_model_only(index)
                    .is_some_and(|queue| {
                        request.ticket.request_index as usize == index
                            && usize::from(request.ticket.queue_ordinal) == queue
                            && request.ticket.queue_id == self.plan.queue_ids[queue]
                            && request.ticket.engine
                                == R48PhysicalEngineLabelV1::for_queue_ordinal_model_only(queue)
                            && request.ticket.submission_epoch == self.plan.submission_epoch
                            && usize::from(request.ticket.ring_slot) < R48_RING_SLOT_COUNT_V1
                            && request.publication.is_exact_for_model_only(request.ticket)
                            && !self.requests[..index].iter().any(|prior| {
                                (prior.ticket.queue_ordinal == request.ticket.queue_ordinal
                                    && (prior.publication.packet_occurrence
                                        == request.publication.packet_occurrence
                                        || prior.ticket.ring_slot == request.ticket.ring_slot))
                                    || prior.ticket.signal == request.ticket.signal
                            })
                    })
            })
            && self.tails.iter().enumerate().all(|(slot, tail)| {
                let last_request = slot
                    + ((self.requests.len() - 1 - slot) / self.plan.queue_count_model_only())
                        * self.plan.queue_count_model_only();
                tail.normalized_queue_slot as usize == slot
                    && self.requests.get(last_request).is_some_and(|request| {
                        tail.ticket == request.ticket && tail.publication == request.publication
                    })
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R48CurrentnessEnvelopeV1 {
    pub owner_occurrence: u64,
    pub session_occurrence: u64,
    pub submission_epoch: u64,
    pub queue_generation: u64,
    pub wait_epoch: u64,
    pub opening_current: bool,
    pub closing_current: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R48CompletionStateV1 {
    Ready,
    Pending,
    Error { terminal_token: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R48TailObservationV1 {
    pub tail: R48BoundTailV1,
    pub state: R48CompletionStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R48AuditObservationV1 {
    pub ticket: R48StripedTicketV1,
    pub state: R48CompletionStateV1,
    pub retirement_ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R48ModelRetakeV1 {
    Succeeds,
    Fails { terminal_token: NonZeroU64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R48WaitWorkV1 {
    pub wait_epoch: u64,
    pub completed_tail_rounds: u64,
    pub tail_load_attempts: u64,
    pub audit_load_attempts: u16,
    pub cumulative_tail_load_attempts: u64,
    pub cumulative_audit_load_attempts: u64,
    pub retired_count: u16,
    pub post_bind_allocation_events: u8,
}

impl R48WaitWorkV1 {
    pub fn complete_round_formula_model_only(self, active_shards: usize) -> bool {
        self.completed_tail_rounds
            .checked_mul(active_shards as u64)
            .is_some_and(|complete| complete <= self.tail_load_attempts)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R48TailWaitErrorV1 {
    InvalidSubmissionIdentity,
    SubmissionEpochExhausted,
    InvalidProfile,
    InvalidCursor,
    RequestCapacity,
    PublicationEvidence,
    InvalidCurrentness,
    TailRoster,
    TailObservation { attempted: u16, terminal_token: u64 },
    ObservationCounterOverflow,
    AuditRoster,
    AuditObservation { attempted: u16, terminal_token: u64 },
    TailReadyWithPendingPrefix { request_index: u16 },
    RetirementPreflight { request_index: u16 },
    ClosingCurrentness,
    WaitEpochExhausted,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R48RetryableStripedTailWaitV1 {
    submission: R48PublishedStripedSubmissionV1,
    work: R48WaitWorkV1,
}

impl R48RetryableStripedTailWaitV1 {
    pub fn bind_model_only(submission: R48PublishedStripedSubmissionV1) -> Self {
        Self {
            submission,
            work: R48WaitWorkV1 {
                wait_epoch: 1,
                completed_tail_rounds: 0,
                tail_load_attempts: 0,
                audit_load_attempts: 0,
                cumulative_tail_load_attempts: 0,
                cumulative_audit_load_attempts: 0,
                retired_count: 0,
                post_bind_allocation_events: 0,
            },
        }
    }

    pub const fn submission_model_only(&self) -> &R48PublishedStripedSubmissionV1 {
        &self.submission
    }

    pub const fn work_model_only(&self) -> R48WaitWorkV1 {
        self.work
    }

    pub fn is_exact_model_only(&self) -> bool {
        self.submission.is_exact_model_only()
            && self.work.wait_epoch != 0
            && self
                .work
                .complete_round_formula_model_only(self.submission.tails.len())
            && self.work.audit_load_attempts == 0
            && self.work.retired_count == 0
            && self.work.post_bind_allocation_events == 0
    }

    pub fn wait_round_model_only(
        mut self,
        currentness: R48CurrentnessEnvelopeV1,
        tails: &[R48TailObservationV1],
        final_audit: &[R48AuditObservationV1],
        deadline_reached: bool,
        retake: R48ModelRetakeV1,
    ) -> R48TailWaitOutcomeV1 {
        if !self.currentness_identity_is_exact_model_only(currentness)
            || !currentness.opening_current
        {
            return self.terminal_model_only(R48TailWaitErrorV1::InvalidCurrentness);
        }

        let mut active_tail_queue_mask = 0_u16;
        let mut ready_tail_queue_mask = 0_u16;
        for index in 0..self.submission.tails.len() {
            let Some(observation) = tails.get(index) else {
                return self.terminal_model_only(R48TailWaitErrorV1::TailRoster);
            };
            if observation.tail != self.submission.tails[index] {
                return self.terminal_model_only(R48TailWaitErrorV1::TailRoster);
            }
            if !self.record_tail_load_model_only() {
                return self.terminal_model_only(R48TailWaitErrorV1::ObservationCounterOverflow);
            }
            let Some(queue_bit) = queue_bit_model_only(observation.tail.ticket.queue_ordinal)
            else {
                return self.terminal_model_only(R48TailWaitErrorV1::TailRoster);
            };
            active_tail_queue_mask |= queue_bit;
            match observation.state {
                R48CompletionStateV1::Ready => ready_tail_queue_mask |= queue_bit,
                R48CompletionStateV1::Pending => {}
                R48CompletionStateV1::Error { terminal_token } => {
                    return self.terminal_model_only(R48TailWaitErrorV1::TailObservation {
                        attempted: (index + 1) as u16,
                        terminal_token,
                    });
                }
            }
        }
        if tails.len() != self.submission.tails.len() {
            return self.terminal_model_only(R48TailWaitErrorV1::TailRoster);
        }
        let Some(completed_rounds) = self.work.completed_tail_rounds.checked_add(1) else {
            return self.terminal_model_only(R48TailWaitErrorV1::ObservationCounterOverflow);
        };
        self.work.completed_tail_rounds = completed_rounds;

        let saw_pending_tail = ready_tail_queue_mask != active_tail_queue_mask;
        if saw_pending_tail && !deadline_reached {
            return R48TailWaitOutcomeV1::Pending(R48TailWaitingV1 { owner: self });
        }

        let mut pending_queue_mask = 0_u16;
        let mut final_ready_tail_queue_mask = 0_u16;
        let mut first_preflight = None;
        for index in 0..self.submission.requests.len() {
            let Some(observation) = final_audit.get(index) else {
                return self.terminal_model_only(R48TailWaitErrorV1::AuditRoster);
            };
            if observation.ticket != self.submission.requests[index].ticket {
                return self.terminal_model_only(R48TailWaitErrorV1::AuditRoster);
            }
            if !self.record_audit_load_model_only() {
                return self.terminal_model_only(R48TailWaitErrorV1::ObservationCounterOverflow);
            }
            let Some(queue_bit) = queue_bit_model_only(observation.ticket.queue_ordinal) else {
                return self.terminal_model_only(R48TailWaitErrorV1::AuditRoster);
            };
            let is_bound_tail = self
                .submission
                .plan
                .normalized_slot_model_only(usize::from(observation.ticket.queue_ordinal))
                .and_then(|slot| self.submission.tails.get(slot))
                .is_some_and(|tail| tail.ticket == observation.ticket);
            match observation.state {
                R48CompletionStateV1::Ready => {
                    if is_bound_tail {
                        final_ready_tail_queue_mask |= queue_bit;
                    }
                }
                R48CompletionStateV1::Pending => pending_queue_mask |= queue_bit,
                R48CompletionStateV1::Error { terminal_token } => {
                    return self.terminal_model_only(R48TailWaitErrorV1::AuditObservation {
                        attempted: (index + 1) as u16,
                        terminal_token,
                    });
                }
            }
            if !observation.retirement_ready && first_preflight.is_none() {
                first_preflight = Some(observation.ticket.request_index);
            }
        }
        if final_audit.len() != self.submission.requests.len() {
            return self.terminal_model_only(R48TailWaitErrorV1::AuditRoster);
        }
        if !currentness.closing_current {
            return self.terminal_model_only(R48TailWaitErrorV1::ClosingCurrentness);
        }
        let ready_tail_union = ready_tail_queue_mask | final_ready_tail_queue_mask;
        let violating_queue_mask = pending_queue_mask & ready_tail_union;
        if violating_queue_mask != 0 {
            let Some(request_index) = final_audit
                .iter()
                .find(|observation| {
                    observation.state == R48CompletionStateV1::Pending
                        && queue_bit_model_only(observation.ticket.queue_ordinal)
                            .is_some_and(|bit| bit & violating_queue_mask != 0)
                })
                .map(|observation| observation.ticket.request_index)
            else {
                return self.terminal_model_only(R48TailWaitErrorV1::AuditRoster);
            };
            return self.terminal_model_only(R48TailWaitErrorV1::TailReadyWithPendingPrefix {
                request_index,
            });
        }
        if pending_queue_mask != 0 {
            return R48TailWaitOutcomeV1::TimedOut(R48TailTimedOutV1 { owner: self });
        }
        if let Some(request_index) = first_preflight {
            return self
                .terminal_model_only(R48TailWaitErrorV1::RetirementPreflight { request_index });
        }

        self.work.retired_count = self.submission.plan.request_count;
        match retake {
            R48ModelRetakeV1::Succeeds => {
                let completed = self
                    .submission
                    .requests
                    .iter()
                    .map(|request| request.payload_identity)
                    .collect();
                let R48PublishedStripedSubmissionV1 {
                    plan,
                    requests,
                    tails: _,
                } = self.submission;
                R48TailWaitOutcomeV1::Completed(R48TailCompletedV1 {
                    plan,
                    published_requests: requests,
                    completed_payloads: completed,
                    work: self.work,
                })
            }
            R48ModelRetakeV1::Fails { terminal_token } => {
                R48TailWaitOutcomeV1::CompletedOpaque(R48CompletedOpaqueV1 {
                    submission: self.submission,
                    terminal_token: terminal_token.get(),
                    work: self.work,
                })
            }
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn panic_guard_model_only(
        mut self,
        stage: R48PanicStageV1,
    ) -> Result<R48PanicRetainedV1, R48TailTerminalV1> {
        let active = self.submission.tails.len();
        let requests = self.submission.requests.len();
        let (tail_prefix, completed_round, audit_prefix) = match stage {
            R48PanicStageV1::BeforeObservation => (0, false, 0),
            R48PanicStageV1::TailPrefix { attempted } => (usize::from(attempted), false, 0),
            R48PanicStageV1::AuditPrefix { attempted } => (active, true, usize::from(attempted)),
            R48PanicStageV1::AfterClosingCurrentness => (active, true, requests),
        };
        if tail_prefix > active || audit_prefix > requests {
            return Err(self.terminal_value_model_only(R48TailWaitErrorV1::TailRoster));
        }
        let Some(tail_loads) = self.work.tail_load_attempts.checked_add(tail_prefix as u64) else {
            return Err(
                self.terminal_value_model_only(R48TailWaitErrorV1::ObservationCounterOverflow)
            );
        };
        let Some(cumulative_tail) = self
            .work
            .cumulative_tail_load_attempts
            .checked_add(tail_prefix as u64)
        else {
            return Err(
                self.terminal_value_model_only(R48TailWaitErrorV1::ObservationCounterOverflow)
            );
        };
        let Some(audit_loads) = self
            .work
            .audit_load_attempts
            .checked_add(audit_prefix as u16)
        else {
            return Err(
                self.terminal_value_model_only(R48TailWaitErrorV1::ObservationCounterOverflow)
            );
        };
        let Some(cumulative_audit) = self
            .work
            .cumulative_audit_load_attempts
            .checked_add(audit_prefix as u64)
        else {
            return Err(
                self.terminal_value_model_only(R48TailWaitErrorV1::ObservationCounterOverflow)
            );
        };
        self.work.tail_load_attempts = tail_loads;
        self.work.cumulative_tail_load_attempts = cumulative_tail;
        self.work.audit_load_attempts = audit_loads;
        self.work.cumulative_audit_load_attempts = cumulative_audit;
        if completed_round {
            let Some(rounds) = self.work.completed_tail_rounds.checked_add(1) else {
                return Err(
                    self.terminal_value_model_only(R48TailWaitErrorV1::ObservationCounterOverflow)
                );
            };
            self.work.completed_tail_rounds = rounds;
        }
        Ok(R48PanicRetainedV1 { owner: self, stage })
    }

    fn currentness_identity_is_exact_model_only(
        &self,
        currentness: R48CurrentnessEnvelopeV1,
    ) -> bool {
        let plan = &self.submission.plan;
        currentness.owner_occurrence == plan.owner_occurrence
            && currentness.session_occurrence == plan.session_occurrence
            && currentness.submission_epoch == plan.submission_epoch
            && currentness.queue_generation == plan.queue_generation
            && currentness.wait_epoch == self.work.wait_epoch
    }

    fn record_tail_load_model_only(&mut self) -> bool {
        let Some(epoch) = self.work.tail_load_attempts.checked_add(1) else {
            return false;
        };
        let Some(cumulative) = self.work.cumulative_tail_load_attempts.checked_add(1) else {
            return false;
        };
        self.work.tail_load_attempts = epoch;
        self.work.cumulative_tail_load_attempts = cumulative;
        true
    }

    fn record_audit_load_model_only(&mut self) -> bool {
        let Some(epoch) = self.work.audit_load_attempts.checked_add(1) else {
            return false;
        };
        let Some(cumulative) = self.work.cumulative_audit_load_attempts.checked_add(1) else {
            return false;
        };
        self.work.audit_load_attempts = epoch;
        self.work.cumulative_audit_load_attempts = cumulative;
        true
    }

    fn terminal_model_only(self, reason: R48TailWaitErrorV1) -> R48TailWaitOutcomeV1 {
        R48TailWaitOutcomeV1::Terminal(self.terminal_value_model_only(reason))
    }

    fn terminal_value_model_only(self, reason: R48TailWaitErrorV1) -> R48TailTerminalV1 {
        let work = self.work;
        R48TailTerminalV1 {
            owner: self,
            reason,
            work,
        }
    }
}

fn queue_bit_model_only(queue_ordinal: u8) -> Option<u16> {
    1_u16.checked_shl(u32::from(queue_ordinal))
}

#[derive(Debug, Eq, PartialEq)]
pub struct R48TailWaitingV1 {
    owner: R48RetryableStripedTailWaitV1,
}

impl R48TailWaitingV1 {
    pub fn into_owner_model_only(self) -> R48RetryableStripedTailWaitV1 {
        self.owner
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R48TailTimedOutV1 {
    owner: R48RetryableStripedTailWaitV1,
}

impl R48TailTimedOutV1 {
    pub const fn owner_model_only(&self) -> &R48RetryableStripedTailWaitV1 {
        &self.owner
    }

    pub fn is_exact_retryable_custody_model_only(&self) -> bool {
        self.owner.submission.is_exact_model_only()
            && usize::from(self.owner.work.audit_load_attempts)
                == self.owner.submission.requests.len()
            && self.owner.work.retired_count == 0
    }

    #[allow(clippy::result_large_err)]
    pub fn retry_model_only(self) -> Result<R48RetryableStripedTailWaitV1, R48TailTerminalV1> {
        let mut owner = self.owner;
        let Some(next_epoch) = owner.work.wait_epoch.checked_add(1) else {
            return Err(owner.terminal_value_model_only(R48TailWaitErrorV1::WaitEpochExhausted));
        };
        owner.work.wait_epoch = next_epoch;
        owner.work.completed_tail_rounds = 0;
        owner.work.tail_load_attempts = 0;
        owner.work.audit_load_attempts = 0;
        Ok(owner)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R48TailTerminalV1 {
    owner: R48RetryableStripedTailWaitV1,
    pub reason: R48TailWaitErrorV1,
    pub work: R48WaitWorkV1,
}

impl R48TailTerminalV1 {
    pub const fn owner_model_only(&self) -> &R48RetryableStripedTailWaitV1 {
        &self.owner
    }

    pub fn has_zero_partial_retirement_model_only(&self) -> bool {
        self.owner.work.retired_count == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R48PanicStageV1 {
    BeforeObservation,
    TailPrefix { attempted: u16 },
    AuditPrefix { attempted: u16 },
    AfterClosingCurrentness,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R48PanicRetainedV1 {
    owner: R48RetryableStripedTailWaitV1,
    pub stage: R48PanicStageV1,
}

impl R48PanicRetainedV1 {
    pub const fn owner_model_only(&self) -> &R48RetryableStripedTailWaitV1 {
        &self.owner
    }

    pub fn is_exact_guard_custody_model_only(&self) -> bool {
        self.owner.submission.is_exact_model_only() && self.owner.work.retired_count == 0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R48TailCompletedV1 {
    pub plan: R48StripedPlanV1,
    published_requests: Vec<R48PublishedRequestV1>,
    pub completed_payloads: Vec<u64>,
    pub work: R48WaitWorkV1,
}

impl R48TailCompletedV1 {
    pub fn is_exact_model_only(&self) -> bool {
        self.completed_payloads.len() == usize::from(self.plan.request_count)
            && self.published_requests.len() == usize::from(self.plan.request_count)
            && self
                .published_requests
                .iter()
                .zip(&self.completed_payloads)
                .all(|(request, payload)| request.payload_identity == *payload)
            && self.work.retired_count == self.plan.request_count
            && usize::from(self.work.audit_load_attempts) == usize::from(self.plan.request_count)
            && self.work.post_bind_allocation_events == 0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R48CompletedOpaqueV1 {
    submission: R48PublishedStripedSubmissionV1,
    pub terminal_token: u64,
    pub work: R48WaitWorkV1,
}

impl R48CompletedOpaqueV1 {
    pub const fn submission_model_only(&self) -> &R48PublishedStripedSubmissionV1 {
        &self.submission
    }

    pub fn is_exact_terminal_custody_model_only(&self) -> bool {
        self.submission.is_exact_model_only()
            && self.work.retired_count == self.submission.plan.request_count
            && self.terminal_token != 0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R48TailWaitOutcomeV1 {
    Pending(R48TailWaitingV1),
    TimedOut(R48TailTimedOutV1),
    Terminal(R48TailTerminalV1),
    Completed(R48TailCompletedV1),
    CompletedOpaque(R48CompletedOpaqueV1),
}

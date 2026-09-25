//! Independent executable R41 model for persistent striped-SDMA custody.
//!
//! The adapter accepts only an exact R40 combined directional-and-striped
//! queue plan. Each request owns one whole persistent device allocation and
//! one whole host allocation; both identity classes must be pairwise distinct.
//! This deliberately does not model splitting or aliasing one allocation
//! across queues.
//!
//! Preparation validates one session, directional-pair occurrence, queue and
//! pool generations, ranges, and currentness, then prepares the complete
//! ticket/identity roster and recovery, completion, and terminal capacities.
//! A preparation failure either restores every request in input order or
//! quarantines the whole batch. Publication is either complete, or reports an
//! exact prefix of confirmed shards, at most one indeterminate shard, and the
//! untouched suffix. Only complete publication followed by exact currentness
//! closure commits the round-robin cursor.
//!
//! Polling validates the complete presentation before status inspection.
//! Pending and timeout scan and retain the whole submission. Ready completion
//! preflights every identity before restoration; restoration either returns
//! every persistent owner in request order or quarantines every owner in one
//! terminal aggregate. Quarantine has no modeled exit.
//!
//! All values are caller-constructed mathematical inputs. This module performs
//! no I/O and does not refine production Rust, KFD, HSA, HIP, queue ioctls,
//! packets, atomics, clocks, firmware, hardware, progress, parity, or
//! performance. Capacity reservation is an abstract phase property, not an
//! allocator-failure or panic claim.

use alloc::vec::Vec;

use crate::{
    R40_GFX942_COMBINED_MAX_REQUESTS_V1, R40_GFX942_COMBINED_MAX_STRIPED_QUEUES_V1,
    R40_GFX942_COMBINED_MIN_STRIPED_QUEUES_V1, R40_GFX942_REQUESTS_PER_STRIPED_QUEUE_V1,
    R40AggregateTicketV1, R40Gfx942SdmaPlanKindV1, R40Gfx942SdmaQueuePlanV1,
    R40Gfx942SdmaQueueRoleV1,
};

pub const R41_GFX942_MIN_STRIPED_QUEUES_V1: u8 = R40_GFX942_COMBINED_MIN_STRIPED_QUEUES_V1;
pub const R41_GFX942_MAX_STRIPED_QUEUES_V1: u8 = R40_GFX942_COMBINED_MAX_STRIPED_QUEUES_V1;
pub const R41_GFX942_REQUESTS_PER_SHARD_V1: u16 = R40_GFX942_REQUESTS_PER_STRIPED_QUEUE_V1;
pub const R41_GFX942_MAX_REQUESTS_V1: u16 = R40_GFX942_COMBINED_MAX_REQUESTS_V1;
pub const R41_GFX942_MAX_LINEAR_COPY_BYTES_V1: u64 = 0x003f_ffe0;
pub const R41_PERSISTENT_DEVICE_MAX_ALLOCATION_BYTES_V1: u64 = 256 * 1024 * 1024;
pub const R41_GPU_PAGE_BYTES_V1: u64 = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41PersistentSdmaDirectionV1 {
    HostToDevice,
    DeviceToHost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R41DirectionalPairOccurrenceV1 {
    pub session_id: u64,
    pub d2h_queue_id: u32,
    pub h2d_queue_id: u32,
    pub queue_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R41CombinedCurrentnessV1 {
    pub session_id: u64,
    pub directional_pair: R41DirectionalPairOccurrenceV1,
    pub striped_queue_generation: u64,
    pub scope_closed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R41PersistentDeviceIdentityV1 {
    pub owner_id: u64,
    pub storage_id: u64,
    pub session_id: u64,
    pub directional_pair: R41DirectionalPairOccurrenceV1,
    pub pool_generation: u64,
    pub logical_bytes: u64,
    pub physical_bytes: u64,
    pub kind: R41DeviceStorageKindV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41DeviceStorageKindV1 {
    DeviceLocal,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R41HostStorageIdentityV1 {
    pub storage_id: u64,
    pub session_id: u64,
    pub pool_generation: u64,
    pub logical_bytes: u64,
    pub physical_bytes: u64,
    pub kind: R41HostStorageKindV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41HostStorageKindV1 {
    CoherentGtt,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R41PersistentStripedRequestIdentityV1 {
    pub request_token: u64,
    pub direction: R41PersistentSdmaDirectionV1,
    pub device: R41PersistentDeviceIdentityV1,
    pub host: R41HostStorageIdentityV1,
    pub device_offset: u64,
    pub host_offset: u64,
    pub copy_bytes: u64,
}

/// Move-only owner of one modeled persistent device/host copy pair.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R41PersistentStripedRequestV1;
/// let request: R41PersistentStripedRequestV1 = todo!();
/// let duplicate = request.clone();
/// # let _ = duplicate;
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R41PersistentStripedRequestV1 {
    identity: R41PersistentStripedRequestIdentityV1,
}

impl R41PersistentStripedRequestV1 {
    pub fn new_model_only(identity: R41PersistentStripedRequestIdentityV1) -> Self {
        Self { identity }
    }

    pub fn identity_model_only(&self) -> R41PersistentStripedRequestIdentityV1 {
        self.identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R41PersistentStripedPlanV1 {
    pub queue_plan: R40Gfx942SdmaQueuePlanV1,
    pub directional_pair: R41DirectionalPairOccurrenceV1,
    pub admitted_currentness: R41CombinedCurrentnessV1,
    pub submission_id: u64,
    pub started_ns: u64,
    pub deadline_ns: u64,
    pub request_count: u16,
    pub first_queue: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R41CompletionIdentityV1 {
    pub ticket: R40AggregateTicketV1,
    pub request: R41PersistentStripedRequestIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41PreparationRecoveryV1 {
    Succeeds,
    Fails { terminal_token: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41PreparationScriptV1 {
    Succeeds,
    FailsAt {
        request_index: u16,
        recovery: R41PreparationRecoveryV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41PreparationFailureReasonV1 {
    InvalidCombinedQueuePlan,
    DirectionalPairMismatch,
    InvalidCurrentness,
    EmptySubmission,
    CapacityExceeded,
    InvalidCursor,
    InvalidDeadline,
    InvalidRequest { request_index: u16 },
    DeviceOwnerAlias { request_index: u16 },
    DeviceStorageAlias { request_index: u16 },
    HostStorageAlias { request_index: u16 },
    InvalidPreparationScript,
    InjectedPreparationFailure { request_index: u16 },
}

#[derive(Debug, Eq, PartialEq)]
pub struct R41RetryablePreparationV1 {
    requests: Vec<R41PersistentStripedRequestV1>,
    pub reason: R41PreparationFailureReasonV1,
    pub prepared_count: usize,
    pub recovered_count: usize,
    pub cursor_before: u8,
}

impl R41RetryablePreparationV1 {
    pub fn requests_model_only(&self) -> &[R41PersistentStripedRequestV1] {
        &self.requests
    }

    pub fn into_requests_model_only(self) -> Vec<R41PersistentStripedRequestV1> {
        self.requests
    }

    pub fn recovered_every_request_in_order_model_only(&self) -> bool {
        self.recovered_count == self.requests.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41PersistentOwnerStateV1 {
    Prepared,
    Published,
    Settled,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41ShardPublicationClassV1 {
    Confirmed,
    Indeterminate,
    Untouched,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R41ShardPublicationV1 {
    pub queue_slot: u8,
    pub queue_id: u32,
    pub class: R41ShardPublicationClassV1,
    pub request_indices: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R41PublicationPartitionV1 {
    pub shards: Vec<R41ShardPublicationV1>,
}

impl R41PublicationPartitionV1 {
    pub fn confirmed_count_model_only(&self) -> usize {
        self.shards
            .iter()
            .filter(|shard| shard.class == R41ShardPublicationClassV1::Confirmed)
            .count()
    }

    pub fn indeterminate_count_model_only(&self) -> usize {
        self.shards
            .iter()
            .filter(|shard| shard.class == R41ShardPublicationClassV1::Indeterminate)
            .count()
    }

    pub fn untouched_count_model_only(&self) -> usize {
        self.shards
            .iter()
            .filter(|shard| shard.class == R41ShardPublicationClassV1::Untouched)
            .count()
    }

    pub fn is_full_model_only(&self) -> bool {
        !self.shards.is_empty()
            && self
                .shards
                .iter()
                .all(|shard| shard.class == R41ShardPublicationClassV1::Confirmed)
    }

    pub fn is_exact_for_model_only(
        &self,
        plan: &R41PersistentStripedPlanV1,
        roster: &[R41CompletionIdentityV1],
    ) -> bool {
        if self.shards.len() != usize::from(plan.queue_plan.striped_queue_count)
            || self.indeterminate_count_model_only() > 1
        {
            return false;
        }
        let mut phase = 0u8;
        for (slot, shard) in self.shards.iter().enumerate() {
            let expected_queue = match plan.queue_plan.striped_queue_model_only(slot as u8) {
                Some(queue) => queue,
                None => return false,
            };
            if usize::from(shard.queue_slot) != slot || shard.queue_id != expected_queue.queue_id {
                return false;
            }
            phase = match (phase, shard.class) {
                (0, R41ShardPublicationClassV1::Confirmed) => 0,
                (0, R41ShardPublicationClassV1::Indeterminate) => 1,
                (0, R41ShardPublicationClassV1::Untouched) => 2,
                (1, R41ShardPublicationClassV1::Untouched) => 2,
                (2, R41ShardPublicationClassV1::Untouched) => 2,
                _ => return false,
            };
            let expected: Vec<u16> = roster
                .iter()
                .filter(|identity| usize::from(identity.ticket.queue_slot) == slot)
                .map(|identity| identity.ticket.request_index)
                .collect();
            if shard.request_indices != expected {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41PublicationScriptV1 {
    Full,
    StopsAfter {
        confirmed_shards: u8,
        next_shard_indeterminate: bool,
        terminal_token: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41TerminalReasonV1 {
    PreparationRecoveryFailed {
        terminal_token: u64,
    },
    InvalidPublicationScript,
    PublicationStopped {
        terminal_token: u64,
    },
    PublicationCurrentnessCloseFailed,
    PresentationSubstitution,
    CompletionCurrentnessCloseFailed,
    InvalidObservationTime,
    ObservationRosterMismatch,
    ObservationError {
        terminal_token: u64,
    },
    CompletionPreflightMismatch,
    CompletionIdentityPreflightFailed {
        request_index: u16,
    },
    InvalidRestorationScript,
    PersistentRestorationFailed {
        request_index: u16,
        terminal_token: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41TerminalStageV1 {
    Preparation,
    Publication,
    Completion,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R41TerminalEntryV1 {
    request: R41PersistentStripedRequestV1,
    pub ticket: Option<R40AggregateTicketV1>,
    pub publication: Option<R41ShardPublicationClassV1>,
    pub restored_before_terminal: bool,
    pub owner_state: R41PersistentOwnerStateV1,
}

impl R41TerminalEntryV1 {
    pub fn request_model_only(&self) -> &R41PersistentStripedRequestV1 {
        &self.request
    }
}

/// Opaque terminal custody. There is intentionally no method returning its
/// requests or permitting quarantine recovery.
#[derive(Debug, Eq, PartialEq)]
pub struct R41PersistentStripedTerminalV1 {
    pub plan: Option<R41PersistentStripedPlanV1>,
    entries: Vec<R41TerminalEntryV1>,
    pub partition: Option<R41PublicationPartitionV1>,
    pub stage: R41TerminalStageV1,
    pub reason: R41TerminalReasonV1,
    pub observation_count: usize,
    pub restored_count: usize,
    pub cursor_before: u8,
    pub resulting_cursor: u8,
    pub cursor_committed: bool,
}

impl R41PersistentStripedTerminalV1 {
    pub fn entries_model_only(&self) -> &[R41TerminalEntryV1] {
        &self.entries
    }

    pub fn retains_every_owner_in_order_model_only(&self) -> bool {
        self.entries.iter().enumerate().all(|(index, entry)| {
            entry.owner_state == R41PersistentOwnerStateV1::Quarantined
                && self.plan.as_ref().is_none_or(|plan| {
                    index < usize::from(plan.request_count)
                        && entry
                            .ticket
                            .is_none_or(|ticket| usize::from(ticket.request_index) == index)
                })
        }) && self
            .plan
            .as_ref()
            .is_none_or(|plan| self.entries.len() == usize::from(plan.request_count))
    }

    pub fn quarantine_is_monotonic_model_only(&self) -> bool {
        self.retains_every_owner_in_order_model_only()
    }

    pub fn cursor_history_is_consistent_model_only(&self) -> bool {
        if self.cursor_committed {
            self.plan.as_ref().is_some_and(|plan| {
                self.resulting_cursor
                    == ((usize::from(plan.first_queue) + usize::from(plan.request_count))
                        % usize::from(plan.queue_plan.striped_queue_count))
                        as u8
            })
        } else {
            self.resulting_cursor == self.cursor_before
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R41PreparationOutcomeV1 {
    Prepared(R41PersistentStripedPreparedV1),
    Retryable(R41RetryablePreparationV1),
    Terminal(R41PersistentStripedTerminalV1),
}

/// Move-only owner after complete preparation and before publication.
#[derive(Debug, Eq, PartialEq)]
pub struct R41PersistentStripedPreparedV1 {
    plan: R41PersistentStripedPlanV1,
    requests: Vec<R41PersistentStripedRequestV1>,
    completion_roster: Vec<R41CompletionIdentityV1>,
    prepared_recovery_output: Vec<R41PersistentStripedRequestV1>,
    prepared_completed_output: Vec<R41CompletedPersistentCopyV1>,
    prepared_terminal_output: Vec<R41TerminalEntryV1>,
}

impl R41PersistentStripedPreparedV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_model_only(
        queue_plan: R40Gfx942SdmaQueuePlanV1,
        directional_pair: R41DirectionalPairOccurrenceV1,
        opening_currentness: R41CombinedCurrentnessV1,
        submission_id: u64,
        started_ns: u64,
        deadline_ns: u64,
        first_queue: u8,
        requests: Vec<R41PersistentStripedRequestV1>,
        script: R41PreparationScriptV1,
    ) -> R41PreparationOutcomeV1 {
        let failure = validate_admission_model_only(
            &queue_plan,
            directional_pair,
            opening_currentness,
            started_ns,
            deadline_ns,
            first_queue,
            &requests,
        );
        if let Some(reason) = failure {
            let count = requests.len();
            return R41PreparationOutcomeV1::Retryable(R41RetryablePreparationV1 {
                requests,
                reason,
                prepared_count: 0,
                recovered_count: count,
                cursor_before: first_queue,
            });
        }

        let request_count = requests.len() as u16;
        let plan = R41PersistentStripedPlanV1 {
            queue_plan,
            directional_pair,
            admitted_currentness: opening_currentness,
            submission_id,
            started_ns,
            deadline_ns,
            request_count,
            first_queue,
        };

        if let R41PreparationScriptV1::FailsAt {
            request_index,
            recovery,
        } = script
        {
            if request_index >= request_count {
                let count = requests.len();
                return R41PreparationOutcomeV1::Retryable(R41RetryablePreparationV1 {
                    requests,
                    reason: R41PreparationFailureReasonV1::InvalidPreparationScript,
                    prepared_count: 0,
                    recovered_count: count,
                    cursor_before: first_queue,
                });
            }
            let prepared_count = usize::from(request_index);
            return match recovery {
                R41PreparationRecoveryV1::Succeeds => {
                    let count = requests.len();
                    R41PreparationOutcomeV1::Retryable(R41RetryablePreparationV1 {
                        requests,
                        reason: R41PreparationFailureReasonV1::InjectedPreparationFailure {
                            request_index,
                        },
                        prepared_count,
                        recovered_count: count,
                        cursor_before: first_queue,
                    })
                }
                R41PreparationRecoveryV1::Fails { terminal_token } => {
                    R41PreparationOutcomeV1::Terminal(terminal_from_requests_model_only(
                        Some(plan),
                        requests,
                        &[],
                        None,
                        R41TerminalStageV1::Preparation,
                        R41TerminalReasonV1::PreparationRecoveryFailed { terminal_token },
                        0,
                        0,
                        first_queue,
                        first_queue,
                        false,
                        Vec::with_capacity(usize::from(request_count)),
                    ))
                }
            };
        }

        let mut completion_roster = Vec::with_capacity(usize::from(request_count));
        for (index, request) in requests.iter().enumerate() {
            let queue_slot = (usize::from(first_queue) + index)
                % usize::from(plan.queue_plan.striped_queue_count);
            let queue = plan
                .queue_plan
                .striped_queue_model_only(queue_slot as u8)
                .expect("validated combined plan contains every striped slot");
            completion_roster.push(R41CompletionIdentityV1 {
                ticket: R40AggregateTicketV1 {
                    session_id: plan.queue_plan.session_id,
                    submission_id,
                    request_index: index as u16,
                    queue_slot: queue_slot as u8,
                    queue_id: queue.queue_id,
                    queue_generation: opening_currentness.striped_queue_generation,
                },
                request: request.identity_model_only(),
            });
        }
        let prepared = Self {
            plan,
            requests,
            completion_roster,
            prepared_recovery_output: Vec::with_capacity(usize::from(request_count)),
            prepared_completed_output: Vec::with_capacity(usize::from(request_count)),
            prepared_terminal_output: Vec::with_capacity(usize::from(request_count)),
        };
        debug_assert!(prepared.is_fully_prepared_model_only());
        R41PreparationOutcomeV1::Prepared(prepared)
    }

    pub fn plan_model_only(&self) -> &R41PersistentStripedPlanV1 {
        &self.plan
    }

    pub fn requests_model_only(&self) -> &[R41PersistentStripedRequestV1] {
        &self.requests
    }

    pub fn completion_roster_model_only(&self) -> &[R41CompletionIdentityV1] {
        &self.completion_roster
    }

    pub fn is_fully_prepared_model_only(&self) -> bool {
        self.completion_roster.len() == self.requests.len()
            && self
                .completion_roster
                .iter()
                .zip(&self.requests)
                .enumerate()
                .all(|(index, (identity, request))| {
                    usize::from(identity.ticket.request_index) == index
                        && identity.request == request.identity_model_only()
                })
            && self.prepared_recovery_output.is_empty()
            && self.prepared_recovery_output.capacity() >= self.requests.len()
            && self.prepared_completed_output.is_empty()
            && self.prepared_completed_output.capacity() >= self.requests.len()
            && self.prepared_terminal_output.is_empty()
            && self.prepared_terminal_output.capacity() >= self.requests.len()
            && (0..self.plan.queue_plan.striped_queue_count).all(|slot| {
                self.completion_roster
                    .iter()
                    .filter(|identity| identity.ticket.queue_slot == slot)
                    .count()
                    <= usize::from(R41_GFX942_REQUESTS_PER_SHARD_V1)
            })
    }

    pub fn publish_model_only(
        self,
        closing_currentness: R41CombinedCurrentnessV1,
        script: R41PublicationScriptV1,
    ) -> R41PublicationOutcomeV1 {
        let partition =
            match publication_partition_model_only(&self.plan, &self.completion_roster, script) {
                Some(partition) => partition,
                None => {
                    return R41PublicationOutcomeV1::Terminal(self.into_terminal_model_only(
                        None,
                        R41TerminalReasonV1::InvalidPublicationScript,
                    ));
                }
            };
        if !partition.is_full_model_only() {
            let terminal_token = match script {
                R41PublicationScriptV1::StopsAfter { terminal_token, .. } => terminal_token,
                R41PublicationScriptV1::Full => 0,
            };
            return R41PublicationOutcomeV1::Terminal(self.into_terminal_model_only(
                Some(partition),
                R41TerminalReasonV1::PublicationStopped { terminal_token },
            ));
        }
        if closing_currentness != self.plan.admitted_currentness
            || !closing_currentness.scope_closed
        {
            return R41PublicationOutcomeV1::Terminal(self.into_terminal_model_only(
                Some(partition),
                R41TerminalReasonV1::PublicationCurrentnessCloseFailed,
            ));
        }

        let Self {
            plan,
            requests,
            completion_roster,
            prepared_recovery_output: _,
            prepared_completed_output,
            prepared_terminal_output,
        } = self;
        let committed_cursor = ((usize::from(plan.first_queue) + usize::from(plan.request_count))
            % usize::from(plan.queue_plan.striped_queue_count))
            as u8;
        R41PublicationOutcomeV1::Published(R41PersistentStripedSubmissionV1 {
            plan,
            requests,
            completion_roster,
            partition,
            prepared_completed_output,
            prepared_terminal_output,
            committed_cursor,
        })
    }

    fn into_terminal_model_only(
        self,
        partition: Option<R41PublicationPartitionV1>,
        reason: R41TerminalReasonV1,
    ) -> R41PersistentStripedTerminalV1 {
        let Self {
            plan,
            requests,
            completion_roster,
            prepared_recovery_output: _,
            prepared_completed_output: _,
            prepared_terminal_output,
        } = self;
        let cursor_before = plan.first_queue;
        terminal_from_requests_model_only(
            Some(plan),
            requests,
            &completion_roster,
            partition,
            R41TerminalStageV1::Publication,
            reason,
            0,
            0,
            cursor_before,
            cursor_before,
            false,
            prepared_terminal_output,
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R41PublicationOutcomeV1 {
    Published(R41PersistentStripedSubmissionV1),
    Terminal(R41PersistentStripedTerminalV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R41CompletionPresentationV1 {
    pub plan: R41PersistentStripedPlanV1,
    pub completion_roster: Vec<R41CompletionIdentityV1>,
    pub partition: R41PublicationPartitionV1,
    pub closing_currentness: R41CombinedCurrentnessV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41CompletionObservationStateV1 {
    Ready,
    Pending,
    Error { terminal_token: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R41CompletionObservationV1 {
    pub identity: R41CompletionIdentityV1,
    pub state: R41CompletionObservationStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R41RestorationScriptV1 {
    AllSucceed,
    FailsAt {
        request_index: u16,
        terminal_token: u64,
    },
}

/// Move-only owner after full publication and currentness closure.
#[derive(Debug, Eq, PartialEq)]
pub struct R41PersistentStripedSubmissionV1 {
    plan: R41PersistentStripedPlanV1,
    requests: Vec<R41PersistentStripedRequestV1>,
    completion_roster: Vec<R41CompletionIdentityV1>,
    partition: R41PublicationPartitionV1,
    prepared_completed_output: Vec<R41CompletedPersistentCopyV1>,
    prepared_terminal_output: Vec<R41TerminalEntryV1>,
    committed_cursor: u8,
}

impl R41PersistentStripedSubmissionV1 {
    pub fn plan_model_only(&self) -> &R41PersistentStripedPlanV1 {
        &self.plan
    }

    pub fn completion_roster_model_only(&self) -> &[R41CompletionIdentityV1] {
        &self.completion_roster
    }

    pub fn committed_cursor_model_only(&self) -> u8 {
        self.committed_cursor
    }

    pub fn presentation_model_only(&self) -> R41CompletionPresentationV1 {
        R41CompletionPresentationV1 {
            plan: self.plan.clone(),
            completion_roster: self.completion_roster.clone(),
            partition: self.partition.clone(),
            closing_currentness: self.plan.admitted_currentness,
        }
    }

    pub fn poll_model_only(
        self,
        presentation: &R41CompletionPresentationV1,
        observations: &[R41CompletionObservationV1],
        now_ns: u64,
        completion_identity_preflight: &[bool],
        restoration: R41RestorationScriptV1,
    ) -> R41PollOutcomeV1 {
        if now_ns < self.plan.started_ns {
            return R41PollOutcomeV1::Terminal(self.into_terminal_model_only(
                R41TerminalReasonV1::InvalidObservationTime,
                0,
                0,
            ));
        }
        if presentation.plan != self.plan
            || presentation.completion_roster != self.completion_roster
            || presentation.partition != self.partition
        {
            return R41PollOutcomeV1::Terminal(self.into_terminal_model_only(
                R41TerminalReasonV1::PresentationSubstitution,
                0,
                0,
            ));
        }
        if presentation.closing_currentness != self.plan.admitted_currentness
            || !presentation.closing_currentness.scope_closed
        {
            return R41PollOutcomeV1::Terminal(self.into_terminal_model_only(
                R41TerminalReasonV1::CompletionCurrentnessCloseFailed,
                0,
                0,
            ));
        }
        if observations.len() != self.completion_roster.len()
            || observations
                .iter()
                .zip(&self.completion_roster)
                .any(|(observation, identity)| observation.identity != *identity)
        {
            return R41PollOutcomeV1::Terminal(self.into_terminal_model_only(
                R41TerminalReasonV1::ObservationRosterMismatch,
                0,
                0,
            ));
        }

        let mut saw_pending = false;
        for (index, observation) in observations.iter().enumerate() {
            match observation.state {
                R41CompletionObservationStateV1::Ready => {}
                R41CompletionObservationStateV1::Pending => saw_pending = true,
                R41CompletionObservationStateV1::Error { terminal_token } => {
                    return R41PollOutcomeV1::Terminal(self.into_terminal_model_only(
                        R41TerminalReasonV1::ObservationError { terminal_token },
                        index + 1,
                        0,
                    ));
                }
            }
        }

        if saw_pending {
            let observation_count = observations.len();
            let timed_out = now_ns >= self.plan.deadline_ns;
            let waiting = R41PersistentStripedWaitingV1 {
                submission: self,
                observation_count,
                restored_count: 0,
            };
            return if timed_out {
                R41PollOutcomeV1::TimedOut(waiting)
            } else {
                R41PollOutcomeV1::Pending(waiting)
            };
        }

        if completion_identity_preflight.len() != self.completion_roster.len() {
            return R41PollOutcomeV1::Terminal(self.into_terminal_model_only(
                R41TerminalReasonV1::CompletionPreflightMismatch,
                observations.len(),
                0,
            ));
        }
        if let Some(index) = completion_identity_preflight
            .iter()
            .position(|identity_matches| !identity_matches)
        {
            return R41PollOutcomeV1::Terminal(self.into_terminal_model_only(
                R41TerminalReasonV1::CompletionIdentityPreflightFailed {
                    request_index: index as u16,
                },
                observations.len(),
                0,
            ));
        }

        match restoration {
            R41RestorationScriptV1::AllSucceed => {
                let Self {
                    plan,
                    requests,
                    completion_roster,
                    partition: _,
                    mut prepared_completed_output,
                    prepared_terminal_output: _,
                    committed_cursor,
                } = self;
                for (request, identity) in requests.into_iter().zip(completion_roster) {
                    prepared_completed_output.push(R41CompletedPersistentCopyV1 {
                        request,
                        ticket: identity.ticket,
                        owner_state: R41PersistentOwnerStateV1::Settled,
                    });
                }
                R41PollOutcomeV1::Completed(R41PersistentStripedCompletedV1 {
                    plan,
                    completed: prepared_completed_output,
                    observation_count: observations.len(),
                    restored_count: observations.len(),
                    committed_cursor,
                })
            }
            R41RestorationScriptV1::FailsAt {
                request_index,
                terminal_token,
            } => {
                if request_index >= self.plan.request_count {
                    R41PollOutcomeV1::Terminal(self.into_terminal_model_only(
                        R41TerminalReasonV1::InvalidRestorationScript,
                        observations.len(),
                        0,
                    ))
                } else {
                    R41PollOutcomeV1::Terminal(self.into_terminal_model_only(
                        R41TerminalReasonV1::PersistentRestorationFailed {
                            request_index,
                            terminal_token,
                        },
                        observations.len(),
                        usize::from(request_index),
                    ))
                }
            }
        }
    }

    fn into_terminal_model_only(
        self,
        reason: R41TerminalReasonV1,
        observation_count: usize,
        restored_count: usize,
    ) -> R41PersistentStripedTerminalV1 {
        let Self {
            plan,
            requests,
            completion_roster,
            partition,
            prepared_completed_output: _,
            prepared_terminal_output,
            committed_cursor,
        } = self;
        let cursor_before = plan.first_queue;
        terminal_from_requests_model_only(
            Some(plan),
            requests,
            &completion_roster,
            Some(partition),
            R41TerminalStageV1::Completion,
            reason,
            observation_count,
            restored_count,
            cursor_before,
            committed_cursor,
            true,
            prepared_terminal_output,
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R41PersistentStripedWaitingV1 {
    submission: R41PersistentStripedSubmissionV1,
    pub observation_count: usize,
    pub restored_count: usize,
}

impl R41PersistentStripedWaitingV1 {
    pub fn submission_model_only(&self) -> &R41PersistentStripedSubmissionV1 {
        &self.submission
    }

    pub fn into_submission_model_only(self) -> R41PersistentStripedSubmissionV1 {
        self.submission
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R41CompletedPersistentCopyV1 {
    request: R41PersistentStripedRequestV1,
    pub ticket: R40AggregateTicketV1,
    pub owner_state: R41PersistentOwnerStateV1,
}

impl R41CompletedPersistentCopyV1 {
    pub fn request_model_only(&self) -> &R41PersistentStripedRequestV1 {
        &self.request
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R41PersistentStripedCompletedV1 {
    pub plan: R41PersistentStripedPlanV1,
    completed: Vec<R41CompletedPersistentCopyV1>,
    pub observation_count: usize,
    pub restored_count: usize,
    pub committed_cursor: u8,
}

impl R41PersistentStripedCompletedV1 {
    pub fn completed_model_only(&self) -> &[R41CompletedPersistentCopyV1] {
        &self.completed
    }

    pub fn restoration_is_complete_and_ordered_model_only(&self) -> bool {
        self.completed.len() == usize::from(self.plan.request_count)
            && self.restored_count == self.completed.len()
            && self.completed.iter().enumerate().all(|(index, completed)| {
                completed.owner_state == R41PersistentOwnerStateV1::Settled
                    && usize::from(completed.ticket.request_index) == index
                    && completed.ticket.session_id == self.plan.queue_plan.session_id
                    && completed.ticket.submission_id == self.plan.submission_id
                    && completed.request.identity_model_only().request_token
                        == completed.ticket.request_index as u64 + 1
            })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R41PollOutcomeV1 {
    Pending(R41PersistentStripedWaitingV1),
    TimedOut(R41PersistentStripedWaitingV1),
    Completed(R41PersistentStripedCompletedV1),
    Terminal(R41PersistentStripedTerminalV1),
}

fn validate_admission_model_only(
    queue_plan: &R40Gfx942SdmaQueuePlanV1,
    directional_pair: R41DirectionalPairOccurrenceV1,
    currentness: R41CombinedCurrentnessV1,
    started_ns: u64,
    deadline_ns: u64,
    first_queue: u8,
    requests: &[R41PersistentStripedRequestV1],
) -> Option<R41PreparationFailureReasonV1> {
    if !queue_plan.is_exact_model_only()
        || queue_plan.kind != R40Gfx942SdmaPlanKindV1::CombinedDirectionalStriped
        || queue_plan.striped_queue_count < R41_GFX942_MIN_STRIPED_QUEUES_V1
        || queue_plan.striped_queue_count > R41_GFX942_MAX_STRIPED_QUEUES_V1
        || !queue_plan.striped_queue_count.is_multiple_of(2)
    {
        return Some(R41PreparationFailureReasonV1::InvalidCombinedQueuePlan);
    }
    let directional_matches = queue_plan.queues.len() >= 2
        && queue_plan.queues[0].role == R40Gfx942SdmaQueueRoleV1::Directional
        && queue_plan.queues[1].role == R40Gfx942SdmaQueueRoleV1::Directional
        && queue_plan.queues[0].queue_id == directional_pair.d2h_queue_id
        && queue_plan.queues[1].queue_id == directional_pair.h2d_queue_id
        && queue_plan.session_id == directional_pair.session_id
        && directional_pair.queue_generation > 0;
    if !directional_matches {
        return Some(R41PreparationFailureReasonV1::DirectionalPairMismatch);
    }
    if currentness.session_id != queue_plan.session_id
        || currentness.directional_pair != directional_pair
        || currentness.striped_queue_generation == 0
        || !currentness.scope_closed
    {
        return Some(R41PreparationFailureReasonV1::InvalidCurrentness);
    }
    if requests.is_empty() {
        return Some(R41PreparationFailureReasonV1::EmptySubmission);
    }
    if requests.len() > usize::from(queue_plan.request_capacity_model_only())
        || requests.len() > usize::from(R41_GFX942_MAX_REQUESTS_V1)
    {
        return Some(R41PreparationFailureReasonV1::CapacityExceeded);
    }
    if first_queue >= queue_plan.striped_queue_count {
        return Some(R41PreparationFailureReasonV1::InvalidCursor);
    }
    if deadline_ns < started_ns {
        return Some(R41PreparationFailureReasonV1::InvalidDeadline);
    }
    for (index, request) in requests.iter().enumerate() {
        let identity = request.identity_model_only();
        let valid_ranges = identity.copy_bytes > 0
            && identity
                .device_offset
                .checked_add(identity.copy_bytes)
                .is_some_and(|end| end <= identity.device.logical_bytes)
            && identity
                .host_offset
                .checked_add(identity.copy_bytes)
                .is_some_and(|end| end <= identity.host.logical_bytes);
        if identity.request_token != index as u64 + 1
            || identity.device.owner_id == 0
            || identity.device.storage_id == 0
            || identity.host.storage_id == 0
            || identity.device.session_id != queue_plan.session_id
            || identity.host.session_id != queue_plan.session_id
            || identity.device.directional_pair != directional_pair
            || identity.device.pool_generation == 0
            || identity.device.logical_bytes == 0
            || identity.device.physical_bytes < identity.device.logical_bytes
            || identity.device.physical_bytes > R41_PERSISTENT_DEVICE_MAX_ALLOCATION_BYTES_V1
            || !identity
                .device
                .physical_bytes
                .is_multiple_of(R41_GPU_PAGE_BYTES_V1)
            || identity.device.kind != R41DeviceStorageKindV1::DeviceLocal
            || identity.host.pool_generation == 0
            || identity.host.logical_bytes == 0
            || identity.host.physical_bytes < identity.host.logical_bytes
            || identity.host.kind != R41HostStorageKindV1::CoherentGtt
            || identity.copy_bytes > R41_GFX942_MAX_LINEAR_COPY_BYTES_V1
            || !valid_ranges
        {
            return Some(R41PreparationFailureReasonV1::InvalidRequest {
                request_index: index as u16,
            });
        }
        if requests[..index]
            .iter()
            .any(|prior| prior.identity_model_only().device.owner_id == identity.device.owner_id)
        {
            return Some(R41PreparationFailureReasonV1::DeviceOwnerAlias {
                request_index: index as u16,
            });
        }
        if requests[..index].iter().any(|prior| {
            prior.identity_model_only().device.storage_id == identity.device.storage_id
        }) {
            return Some(R41PreparationFailureReasonV1::DeviceStorageAlias {
                request_index: index as u16,
            });
        }
        if requests[..index]
            .iter()
            .any(|prior| prior.identity_model_only().host.storage_id == identity.host.storage_id)
        {
            return Some(R41PreparationFailureReasonV1::HostStorageAlias {
                request_index: index as u16,
            });
        }
    }
    None
}

fn publication_partition_model_only(
    plan: &R41PersistentStripedPlanV1,
    roster: &[R41CompletionIdentityV1],
    script: R41PublicationScriptV1,
) -> Option<R41PublicationPartitionV1> {
    let queue_count = plan.queue_plan.striped_queue_count;
    let (confirmed, indeterminate) = match script {
        R41PublicationScriptV1::Full => (queue_count, false),
        R41PublicationScriptV1::StopsAfter {
            confirmed_shards,
            next_shard_indeterminate,
            ..
        } if confirmed_shards < queue_count => (confirmed_shards, next_shard_indeterminate),
        R41PublicationScriptV1::StopsAfter { .. } => return None,
    };
    let mut shards = Vec::with_capacity(usize::from(queue_count));
    for slot in 0..queue_count {
        let queue = plan.queue_plan.striped_queue_model_only(slot)?;
        let class = if slot < confirmed {
            R41ShardPublicationClassV1::Confirmed
        } else if slot == confirmed && indeterminate {
            R41ShardPublicationClassV1::Indeterminate
        } else {
            R41ShardPublicationClassV1::Untouched
        };
        let request_indices = roster
            .iter()
            .filter(|identity| identity.ticket.queue_slot == slot)
            .map(|identity| identity.ticket.request_index)
            .collect();
        shards.push(R41ShardPublicationV1 {
            queue_slot: slot,
            queue_id: queue.queue_id,
            class,
            request_indices,
        });
    }
    let partition = R41PublicationPartitionV1 { shards };
    partition
        .is_exact_for_model_only(plan, roster)
        .then_some(partition)
}

#[allow(clippy::too_many_arguments)]
fn terminal_from_requests_model_only(
    plan: Option<R41PersistentStripedPlanV1>,
    requests: Vec<R41PersistentStripedRequestV1>,
    roster: &[R41CompletionIdentityV1],
    partition: Option<R41PublicationPartitionV1>,
    stage: R41TerminalStageV1,
    reason: R41TerminalReasonV1,
    observation_count: usize,
    restored_count: usize,
    cursor_before: u8,
    resulting_cursor: u8,
    cursor_committed: bool,
    mut entries: Vec<R41TerminalEntryV1>,
) -> R41PersistentStripedTerminalV1 {
    debug_assert!(entries.is_empty());
    debug_assert!(entries.capacity() >= requests.len());
    for (index, request) in requests.into_iter().enumerate() {
        let ticket = roster.get(index).map(|identity| identity.ticket);
        let publication = ticket.and_then(|ticket| {
            partition.as_ref().and_then(|partition| {
                partition
                    .shards
                    .get(usize::from(ticket.queue_slot))
                    .map(|shard| shard.class)
            })
        });
        entries.push(R41TerminalEntryV1 {
            request,
            ticket,
            publication,
            restored_before_terminal: index < restored_count,
            owner_state: R41PersistentOwnerStateV1::Quarantined,
        });
    }
    R41PersistentStripedTerminalV1 {
        plan,
        entries,
        partition,
        stage,
        reason,
        observation_count,
        restored_count,
        cursor_before,
        resulting_cursor,
        cursor_committed,
    }
}

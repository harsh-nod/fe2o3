//! Standalone executable R56 model for multiplexing logical SDMA lanes over
//! exactly two native queue occurrences.
//!
//! The model admits logical-lane counts 2, 4, 8, 14, and 16 and batches of
//! `2..=126` requests. Request `i` is assigned to logical lane
//! `(cursor + i) % logical_lane_count`, then to native queue `lane % 2`.
//! Each native issue stream is the stable filter of that canonical request
//! order, so it also preserves FIFO order within every logical lane. At most 63
//! packets are assigned to either native queue.
//!
//! Publication order is rotated by the logical cursor: native `cursor % 2` is
//! first and the other native queue is second. Every admitted complete batch
//! activates both native queues and has exactly two write-pointer publications,
//! two doorbells, and two bound tails. Only complete publication followed by
//! exact closing-currentness validation commits the cursor. Zero- and one-request
//! batches reject before preparation. Opening-currentness rejection and a
//! first-native recoverable no-effect result retain a zero-publication prefix.
//! An indeterminate first publication records that exact first-native shard and
//! the untouched second shard at prefix zero. A confirmed first publication
//! followed by a recoverable or indeterminate second result, and a
//! closing-currentness failure after both publications, retain exact terminal
//! custody without committing the cursor.
//!
//! Wait timeout is a later transition over an exact successfully published batch:
//! it retains both publications and tails and preserves the already committed
//! cursor. Release requires the exact completed ticket roster and returns each
//! original request once, in canonical request order. Post-effect panic/unwind is
//! deliberately outside this typed model and requires a concrete fail-stop or
//! separately proved custody implementation.
//!
//! All values and observations are caller-constructed mathematical inputs. This
//! module performs no I/O and does not refine production Rust, allocation,
//! packet encoders, atomics, KFD, HSA, HIP, firmware, hardware, progress,
//! concurrency, parity, or performance.

use alloc::{boxed::Box, vec::Vec};

pub const R56_NATIVE_QUEUE_COUNT_V1: usize = 2;
pub const R56_MIN_REQUESTS_V1: usize = 2;
pub const R56_MAX_REQUESTS_PER_NATIVE_V1: usize = 63;
pub const R56_MAX_REQUESTS_V1: usize = 126;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R56MuxPresentationLabelV1 {
    LogicalLanesOverTwoNativeQueues,
    FalsePhysicalStriped2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R56NativeQueueIdentityV1 {
    pub session_occurrence: u64,
    pub native_ordinal: u8,
    pub engine_index: u8,
    pub queue_id: u32,
    pub queue_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R56MuxPlanV1 {
    pub label: R56MuxPresentationLabelV1,
    pub owner_occurrence: u64,
    pub session_occurrence: u64,
    pub submission_epoch: u64,
    pub logical_lane_count: u8,
    pub cursor: u8,
    pub native_queues: [R56NativeQueueIdentityV1; R56_NATIVE_QUEUE_COUNT_V1],
}

impl R56MuxPlanV1 {
    pub const fn admits_logical_lane_count_model_only(count: u8) -> bool {
        matches!(count, 2 | 4 | 8 | 14 | 16)
    }

    pub fn is_exact_model_only(&self) -> bool {
        if self.label != R56MuxPresentationLabelV1::LogicalLanesOverTwoNativeQueues
            || self.owner_occurrence == 0
            || self.session_occurrence == 0
            || self.submission_epoch == 0
            || !Self::admits_logical_lane_count_model_only(self.logical_lane_count)
            || self.cursor >= self.logical_lane_count
        {
            return false;
        }
        for (index, queue) in self.native_queues.iter().enumerate() {
            if queue.session_occurrence != self.session_occurrence
                || usize::from(queue.native_ordinal) != index
                || usize::from(queue.engine_index) != index
                || queue.queue_generation == 0
            {
                return false;
            }
        }
        self.native_queues[0].queue_id != self.native_queues[1].queue_id
    }

    pub fn lane_for_request_model_only(&self, request_index: usize) -> Option<u8> {
        if !self.is_exact_model_only() || request_index >= R56_MAX_REQUESTS_V1 {
            return None;
        }
        Some(
            ((usize::from(self.cursor) + request_index) % usize::from(self.logical_lane_count))
                as u8,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R56RequestIdentityV1 {
    pub owner_occurrence: u64,
    pub session_occurrence: u64,
    pub submission_epoch: u64,
    pub request_token: u64,
    pub payload_identity: u64,
}

/// Move-only custody for one modeled request.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R56MuxRequestV1;
/// let request: R56MuxRequestV1 = todo!();
/// let duplicate = request.clone();
/// # let _ = duplicate;
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R56MuxRequestV1 {
    identity: R56RequestIdentityV1,
}

impl R56MuxRequestV1 {
    pub const fn new_model_only(identity: R56RequestIdentityV1) -> Self {
        Self { identity }
    }

    pub const fn identity_model_only(&self) -> R56RequestIdentityV1 {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R56MuxTicketV1 {
    pub owner_occurrence: u64,
    pub session_occurrence: u64,
    pub submission_epoch: u64,
    pub request_index: u16,
    pub request_token: u64,
    pub logical_lane: u8,
    pub native_ordinal: u8,
    pub engine_index: u8,
    pub queue_id: u32,
    pub ring_slot: u8,
    pub queue_generation: u64,
    pub packet_identity: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R56NativeIssueOrderV1 {
    pub native_ordinal: u8,
    pub request_indices: Vec<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R56PreparationErrorV1 {
    InvalidPlan,
    EmptySubmission,
    RequestCountTooSmall,
    RequestCapacityExceeded,
    InvalidRequest { request_index: u16 },
    DuplicateRequestToken { request_index: u16 },
    DuplicatePayloadIdentity { request_index: u16 },
    PacketIdentityOverflow,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R56PreparationFailureV1 {
    pub plan: R56MuxPlanV1,
    requests: Vec<R56MuxRequestV1>,
    pub reason: R56PreparationErrorV1,
}

impl R56PreparationFailureV1 {
    pub fn request_identities_model_only(&self) -> Vec<R56RequestIdentityV1> {
        request_identities(&self.requests)
    }

    pub fn into_requests_model_only(self) -> Vec<R56MuxRequestV1> {
        self.requests
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R56PreparedMuxBatchV1 {
    pub plan: R56MuxPlanV1,
    requests: Vec<R56MuxRequestV1>,
    pub tickets: Vec<R56MuxTicketV1>,
    pub native_orders: Vec<R56NativeIssueOrderV1>,
}

impl R56PreparedMuxBatchV1 {
    pub fn request_identities_model_only(&self) -> Vec<R56RequestIdentityV1> {
        request_identities(&self.requests)
    }

    pub fn active_native_count_model_only(&self) -> usize {
        self.native_orders
            .iter()
            .filter(|order| !order.request_indices.is_empty())
            .count()
    }

    pub fn native_load_model_only(&self, native_ordinal: u8) -> usize {
        self.native_orders
            .iter()
            .find(|order| order.native_ordinal == native_ordinal)
            .map_or(0, |order| order.request_indices.len())
    }

    pub fn is_exact_model_only(&self) -> bool {
        if !self.plan.is_exact_model_only()
            || self.requests.len() < R56_MIN_REQUESTS_V1
            || self.requests.len() > R56_MAX_REQUESTS_V1
            || self.tickets.len() != self.requests.len()
            || self.native_orders.len() != R56_NATIVE_QUEUE_COUNT_V1
        {
            return false;
        }

        for (index, request) in self.requests.iter().enumerate() {
            let identity = request.identity_model_only();
            if identity.owner_occurrence != self.plan.owner_occurrence
                || identity.session_occurrence != self.plan.session_occurrence
                || identity.submission_epoch != self.plan.submission_epoch
                || identity.request_token == 0
                || identity.payload_identity == 0
                || self.requests[..index].iter().any(|prior| {
                    let prior = prior.identity_model_only();
                    prior.request_token == identity.request_token
                        || prior.payload_identity == identity.payload_identity
                })
            {
                return false;
            }
            let Some(expected) = ticket_for_request(&self.plan, identity, index) else {
                return false;
            };
            if self.tickets[index] != expected {
                return false;
            }
        }

        for native in 0..R56_NATIVE_QUEUE_COUNT_V1 {
            let order = &self.native_orders[native];
            if usize::from(order.native_ordinal) != native
                || order.request_indices.len() > R56_MAX_REQUESTS_PER_NATIVE_V1
            {
                return false;
            }
            let expected: Vec<u16> = self
                .tickets
                .iter()
                .filter(|ticket| usize::from(ticket.native_ordinal) == native)
                .map(|ticket| ticket.request_index)
                .collect();
            if order.request_indices != expected {
                return false;
            }
        }
        true
    }
}

pub fn r56_prepare_mux_batch_model_only(
    plan: R56MuxPlanV1,
    requests: Vec<R56MuxRequestV1>,
) -> Result<R56PreparedMuxBatchV1, R56PreparationFailureV1> {
    let reason = if !plan.is_exact_model_only() {
        Some(R56PreparationErrorV1::InvalidPlan)
    } else if requests.is_empty() {
        Some(R56PreparationErrorV1::EmptySubmission)
    } else if requests.len() < R56_MIN_REQUESTS_V1 {
        Some(R56PreparationErrorV1::RequestCountTooSmall)
    } else if requests.len() > R56_MAX_REQUESTS_V1 {
        Some(R56PreparationErrorV1::RequestCapacityExceeded)
    } else {
        validate_requests(&plan, &requests)
    };
    if let Some(reason) = reason {
        return Err(R56PreparationFailureV1 {
            plan,
            requests,
            reason,
        });
    }

    let mut tickets = Vec::with_capacity(requests.len());
    let mut native_orders = (0..R56_NATIVE_QUEUE_COUNT_V1)
        .map(|native| R56NativeIssueOrderV1 {
            native_ordinal: native as u8,
            request_indices: Vec::with_capacity(R56_MAX_REQUESTS_PER_NATIVE_V1),
        })
        .collect::<Vec<_>>();
    for (index, request) in requests.iter().enumerate() {
        let ticket = ticket_for_request(&plan, request.identity_model_only(), index)
            .expect("validated request count and packet identity");
        native_orders[usize::from(ticket.native_ordinal)]
            .request_indices
            .push(ticket.request_index);
        tickets.push(ticket);
    }
    let prepared = R56PreparedMuxBatchV1 {
        plan,
        requests,
        tickets,
        native_orders,
    };
    debug_assert!(prepared.is_exact_model_only());
    Ok(prepared)
}

fn validate_requests(
    plan: &R56MuxPlanV1,
    requests: &[R56MuxRequestV1],
) -> Option<R56PreparationErrorV1> {
    for (index, request) in requests.iter().enumerate() {
        let identity = request.identity_model_only();
        if identity.owner_occurrence != plan.owner_occurrence
            || identity.session_occurrence != plan.session_occurrence
            || identity.submission_epoch != plan.submission_epoch
            || identity.request_token == 0
            || identity.payload_identity == 0
        {
            return Some(R56PreparationErrorV1::InvalidRequest {
                request_index: index as u16,
            });
        }
        if requests[..index]
            .iter()
            .any(|prior| prior.identity_model_only().request_token == identity.request_token)
        {
            return Some(R56PreparationErrorV1::DuplicateRequestToken {
                request_index: index as u16,
            });
        }
        if requests[..index]
            .iter()
            .any(|prior| prior.identity_model_only().payload_identity == identity.payload_identity)
        {
            return Some(R56PreparationErrorV1::DuplicatePayloadIdentity {
                request_index: index as u16,
            });
        }
        if plan
            .submission_epoch
            .checked_mul(1_000)
            .and_then(|base| base.checked_add(index as u64 + 1))
            .is_none()
        {
            return Some(R56PreparationErrorV1::PacketIdentityOverflow);
        }
    }
    None
}

fn ticket_for_request(
    plan: &R56MuxPlanV1,
    request: R56RequestIdentityV1,
    request_index: usize,
) -> Option<R56MuxTicketV1> {
    let logical_lane = plan.lane_for_request_model_only(request_index)?;
    let native_ordinal = logical_lane % R56_NATIVE_QUEUE_COUNT_V1 as u8;
    let native = plan.native_queues[usize::from(native_ordinal)];
    let ring_slot = (0..request_index)
        .filter(|prior| {
            plan.lane_for_request_model_only(*prior)
                .is_some_and(|lane| lane % R56_NATIVE_QUEUE_COUNT_V1 as u8 == native_ordinal)
        })
        .count();
    if ring_slot >= R56_MAX_REQUESTS_PER_NATIVE_V1 {
        return None;
    }
    Some(R56MuxTicketV1 {
        owner_occurrence: request.owner_occurrence,
        session_occurrence: request.session_occurrence,
        submission_epoch: request.submission_epoch,
        request_index: request_index as u16,
        request_token: request.request_token,
        logical_lane,
        native_ordinal,
        engine_index: native.engine_index,
        queue_id: native.queue_id,
        ring_slot: ring_slot as u8,
        queue_generation: native.queue_generation,
        packet_identity: plan
            .submission_epoch
            .checked_mul(1_000)?
            .checked_add(request_index as u64 + 1)?,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R56MuxCurrentnessV1 {
    pub owner_occurrence: u64,
    pub session_occurrence: u64,
    pub submission_epoch: u64,
    pub queue_generations: [u64; R56_NATIVE_QUEUE_COUNT_V1],
    pub opening_current: bool,
}

impl R56MuxCurrentnessV1 {
    fn opening_is_exact_for_model_only(&self, plan: &R56MuxPlanV1) -> bool {
        self.owner_occurrence == plan.owner_occurrence
            && self.session_occurrence == plan.session_occurrence
            && self.submission_epoch == plan.submission_epoch
            && self.queue_generations
                == [
                    plan.native_queues[0].queue_generation,
                    plan.native_queues[1].queue_generation,
                ]
            && self.opening_current
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R56MuxPublicationScriptV1 {
    Complete,
    RecoverableFirstNativeNoEffect,
    IndeterminateFirstNative,
    RecoverableSecondNativeAfterConfirmedFirst,
    IndeterminateSecondNativeAfterConfirmedFirst,
    CompleteThenClosingCurrentnessFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R56NativePublicationV1 {
    pub native_ordinal: u8,
    pub engine_index: u8,
    pub queue_id: u32,
    pub queue_generation: u64,
    pub request_indices: Vec<u16>,
    pub tail: R56MuxTicketV1,
    pub write_pointer_published_release: bool,
    pub doorbell_published_release: bool,
    pub tail_bound_before_doorbell: bool,
}

impl R56NativePublicationV1 {
    fn is_exact_for_model_only(
        &self,
        prepared: &R56PreparedMuxBatchV1,
        native_ordinal: usize,
    ) -> bool {
        let queue = prepared.plan.native_queues[native_ordinal];
        let order = &prepared.native_orders[native_ordinal];
        let Some(last_index) = order.request_indices.last().copied() else {
            return false;
        };
        self.native_ordinal == native_ordinal as u8
            && self.engine_index == queue.engine_index
            && self.queue_id == queue.queue_id
            && self.queue_generation == queue.queue_generation
            && self.request_indices == order.request_indices
            && self.tail == prepared.tickets[usize::from(last_index)]
            && self.write_pointer_published_release
            && self.doorbell_published_release
            && self.tail_bound_before_doorbell
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R56QuarantineReasonV1 {
    InvalidPreparedPresentation,
    OpeningCurrentnessRejected,
    IndeterminateFirstNative,
    RecoverableSecondNativeAfterConfirmedFirst,
    IndeterminateSecondNativeAfterConfirmedFirst,
    ClosingCurrentnessFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R56RetentionReasonV1 {
    RecoverableNoNativeEffect,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R56RetainedMuxBatchV1 {
    plan: R56MuxPlanV1,
    requests: Vec<R56MuxRequestV1>,
    pub reason: R56RetentionReasonV1,
    pub published_native_prefix: u8,
    pub cursor_before: u8,
    pub cursor_committed: bool,
}

impl R56RetainedMuxBatchV1 {
    pub fn request_identities_model_only(&self) -> Vec<R56RequestIdentityV1> {
        request_identities(&self.requests)
    }

    pub fn is_exact_retryable_model_only(&self) -> bool {
        self.reason == R56RetentionReasonV1::RecoverableNoNativeEffect
            && self.plan.is_exact_model_only()
            && (R56_MIN_REQUESTS_V1..=R56_MAX_REQUESTS_V1).contains(&self.requests.len())
            && validate_requests(&self.plan, &self.requests).is_none()
            && self.published_native_prefix == 0
            && self.cursor_before == self.plan.cursor
            && !self.cursor_committed
    }

    pub fn into_requests_model_only(self) -> Vec<R56MuxRequestV1> {
        self.requests
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R56QuarantinedMuxBatchV1 {
    prepared: R56PreparedMuxBatchV1,
    pub reason: R56QuarantineReasonV1,
    pub published_native_prefix: u8,
    pub confirmed_publications: Vec<R56NativePublicationV1>,
    pub indeterminate_publication: Option<R56NativePublicationV1>,
    pub untouched_request_indices: Vec<u16>,
    pub cursor_before: u8,
    pub cursor_committed: bool,
}

impl R56QuarantinedMuxBatchV1 {
    pub fn request_identities_model_only(&self) -> Vec<R56RequestIdentityV1> {
        self.prepared.request_identities_model_only()
    }

    pub fn indeterminate_tickets_model_only(&self) -> Vec<R56MuxTicketV1> {
        self.indeterminate_publication
            .as_ref()
            .into_iter()
            .flat_map(|publication| publication.request_indices.iter())
            .filter_map(|index| self.prepared.tickets.get(usize::from(*index)).copied())
            .collect()
    }

    pub fn untouched_tickets_model_only(&self) -> Vec<R56MuxTicketV1> {
        self.untouched_request_indices
            .iter()
            .filter_map(|index| self.prepared.tickets.get(usize::from(*index)).copied())
            .collect()
    }

    pub fn is_exact_progress_model_only(&self) -> bool {
        if !self.prepared.is_exact_model_only()
            || self.cursor_before != self.prepared.plan.cursor
            || self.cursor_committed
            || usize::from(self.published_native_prefix) != self.confirmed_publications.len()
        {
            return false;
        }
        let publications = build_publications(&self.prepared);
        let all_indices = canonical_request_indices(&self.prepared);
        match self.reason {
            R56QuarantineReasonV1::InvalidPreparedPresentation => false,
            R56QuarantineReasonV1::OpeningCurrentnessRejected => {
                self.confirmed_publications.is_empty()
                    && self.indeterminate_publication.is_none()
                    && self.untouched_request_indices == all_indices
            }
            R56QuarantineReasonV1::IndeterminateFirstNative => {
                self.confirmed_publications.is_empty()
                    && self.indeterminate_publication.as_ref() == publications.first()
                    && self.untouched_request_indices == publications[1].request_indices
            }
            R56QuarantineReasonV1::RecoverableSecondNativeAfterConfirmedFirst => {
                self.confirmed_publications == publications[..1]
                    && self.indeterminate_publication.is_none()
                    && self.untouched_request_indices == publications[1].request_indices
            }
            R56QuarantineReasonV1::IndeterminateSecondNativeAfterConfirmedFirst => {
                self.confirmed_publications == publications[..1]
                    && self.indeterminate_publication.as_ref() == publications.get(1)
                    && self.untouched_request_indices.is_empty()
            }
            R56QuarantineReasonV1::ClosingCurrentnessFailure => {
                self.confirmed_publications == publications
                    && self.indeterminate_publication.is_none()
                    && self.untouched_request_indices.is_empty()
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R56PublishedMuxBatchV1 {
    prepared: R56PreparedMuxBatchV1,
    pub publications: Vec<R56NativePublicationV1>,
    pub cursor_before: u8,
    pub cursor_after: u8,
    pub cursor_committed: bool,
}

impl R56PublishedMuxBatchV1 {
    pub fn request_identities_model_only(&self) -> Vec<R56RequestIdentityV1> {
        self.prepared.request_identities_model_only()
    }

    pub fn tickets_model_only(&self) -> &[R56MuxTicketV1] {
        &self.prepared.tickets
    }

    pub fn write_pointer_publication_count_model_only(&self) -> usize {
        self.publications
            .iter()
            .filter(|publication| publication.write_pointer_published_release)
            .count()
    }

    pub fn doorbell_count_model_only(&self) -> usize {
        self.publications
            .iter()
            .filter(|publication| publication.doorbell_published_release)
            .count()
    }

    pub fn tail_count_model_only(&self) -> usize {
        self.publications.len()
    }

    pub fn is_exact_model_only(&self) -> bool {
        if !self.prepared.is_exact_model_only()
            || self.publications.len() != self.prepared.active_native_count_model_only()
        {
            return false;
        }
        let first_native = usize::from(self.cursor_before) % R56_NATIVE_QUEUE_COUNT_V1;
        for (position, publication) in self.publications.iter().enumerate() {
            let native = usize::from(publication.native_ordinal);
            if native >= R56_NATIVE_QUEUE_COUNT_V1
                || native != (first_native + position) % R56_NATIVE_QUEUE_COUNT_V1
                || !publication.is_exact_for_model_only(&self.prepared, native)
            {
                return false;
            }
        }
        let qualifies = self.prepared.active_native_count_model_only() == R56_NATIVE_QUEUE_COUNT_V1;
        let expected_cursor = ((usize::from(self.cursor_before) + self.prepared.tickets.len())
            % usize::from(self.prepared.plan.logical_lane_count))
            as u8;
        self.cursor_before == self.prepared.plan.cursor
            && self.cursor_committed == qualifies
            && self.cursor_after == expected_cursor
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R56MuxPublishOutcomeV1 {
    Published(R56PublishedMuxBatchV1),
    Retained(R56RetainedMuxBatchV1),
    Quarantined(R56QuarantinedMuxBatchV1),
}

pub fn r56_publish_mux_batch_model_only(
    prepared: R56PreparedMuxBatchV1,
    currentness: R56MuxCurrentnessV1,
    script: R56MuxPublicationScriptV1,
) -> R56MuxPublishOutcomeV1 {
    let cursor_before = prepared.plan.cursor;
    if !prepared.is_exact_model_only() {
        return quarantined(
            prepared,
            R56QuarantineReasonV1::InvalidPreparedPresentation,
            0,
            cursor_before,
        );
    }
    if !currentness.opening_is_exact_for_model_only(&prepared.plan) {
        let untouched_request_indices = canonical_request_indices(&prepared);
        return R56MuxPublishOutcomeV1::Quarantined(R56QuarantinedMuxBatchV1 {
            prepared,
            reason: R56QuarantineReasonV1::OpeningCurrentnessRejected,
            published_native_prefix: 0,
            confirmed_publications: Vec::new(),
            indeterminate_publication: None,
            untouched_request_indices,
            cursor_before,
            cursor_committed: false,
        });
    }
    match script {
        R56MuxPublicationScriptV1::RecoverableFirstNativeNoEffect => {
            let R56PreparedMuxBatchV1 { plan, requests, .. } = prepared;
            R56MuxPublishOutcomeV1::Retained(R56RetainedMuxBatchV1 {
                plan,
                requests,
                reason: R56RetentionReasonV1::RecoverableNoNativeEffect,
                published_native_prefix: 0,
                cursor_before,
                cursor_committed: false,
            })
        }
        R56MuxPublicationScriptV1::IndeterminateFirstNative => {
            let mut publications = build_publications(&prepared).into_iter();
            let first = publications.next().expect("exact batch has a first native");
            let second = publications
                .next()
                .expect("exact batch has a second native");
            R56MuxPublishOutcomeV1::Quarantined(R56QuarantinedMuxBatchV1 {
                prepared,
                reason: R56QuarantineReasonV1::IndeterminateFirstNative,
                published_native_prefix: 0,
                confirmed_publications: Vec::new(),
                indeterminate_publication: Some(first),
                untouched_request_indices: second.request_indices,
                cursor_before,
                cursor_committed: false,
            })
        }
        R56MuxPublicationScriptV1::RecoverableSecondNativeAfterConfirmedFirst => {
            let mut publications = build_publications(&prepared).into_iter();
            let first = publications.next().expect("exact batch has a first native");
            let second = publications
                .next()
                .expect("exact batch has a second native");
            R56MuxPublishOutcomeV1::Quarantined(R56QuarantinedMuxBatchV1 {
                prepared,
                reason: R56QuarantineReasonV1::RecoverableSecondNativeAfterConfirmedFirst,
                published_native_prefix: 1,
                confirmed_publications: alloc::vec![first],
                indeterminate_publication: None,
                untouched_request_indices: second.request_indices,
                cursor_before,
                cursor_committed: false,
            })
        }
        R56MuxPublicationScriptV1::IndeterminateSecondNativeAfterConfirmedFirst => {
            let mut publications = build_publications(&prepared).into_iter();
            let first = publications.next().expect("exact batch has a first native");
            let second = publications
                .next()
                .expect("exact batch has a second native");
            R56MuxPublishOutcomeV1::Quarantined(R56QuarantinedMuxBatchV1 {
                prepared,
                reason: R56QuarantineReasonV1::IndeterminateSecondNativeAfterConfirmedFirst,
                published_native_prefix: 1,
                confirmed_publications: alloc::vec![first],
                indeterminate_publication: Some(second),
                untouched_request_indices: Vec::new(),
                cursor_before,
                cursor_committed: false,
            })
        }
        R56MuxPublicationScriptV1::CompleteThenClosingCurrentnessFailure => {
            let confirmed_publications = build_publications(&prepared);
            R56MuxPublishOutcomeV1::Quarantined(R56QuarantinedMuxBatchV1 {
                prepared,
                reason: R56QuarantineReasonV1::ClosingCurrentnessFailure,
                published_native_prefix: 2,
                confirmed_publications,
                indeterminate_publication: None,
                untouched_request_indices: Vec::new(),
                cursor_before,
                cursor_committed: false,
            })
        }
        R56MuxPublicationScriptV1::Complete => {
            let publications = build_publications(&prepared);
            let cursor_committed =
                prepared.active_native_count_model_only() == R56_NATIVE_QUEUE_COUNT_V1;
            let cursor_after = ((usize::from(cursor_before) + prepared.tickets.len())
                % usize::from(prepared.plan.logical_lane_count))
                as u8;
            let published = R56PublishedMuxBatchV1 {
                prepared,
                publications,
                cursor_before,
                cursor_after,
                cursor_committed,
            };
            debug_assert!(published.is_exact_model_only());
            R56MuxPublishOutcomeV1::Published(published)
        }
    }
}

fn build_publications(prepared: &R56PreparedMuxBatchV1) -> Vec<R56NativePublicationV1> {
    let first_native = usize::from(prepared.plan.cursor) % R56_NATIVE_QUEUE_COUNT_V1;
    (0..R56_NATIVE_QUEUE_COUNT_V1)
        .filter_map(|position| {
            let native = (first_native + position) % R56_NATIVE_QUEUE_COUNT_V1;
            let order = &prepared.native_orders[native];
            let last = usize::from(*order.request_indices.last()?);
            let queue = prepared.plan.native_queues[native];
            Some(R56NativePublicationV1 {
                native_ordinal: native as u8,
                engine_index: queue.engine_index,
                queue_id: queue.queue_id,
                queue_generation: queue.queue_generation,
                request_indices: order.request_indices.clone(),
                tail: prepared.tickets[last],
                write_pointer_published_release: true,
                doorbell_published_release: true,
                tail_bound_before_doorbell: true,
            })
        })
        .collect()
}

fn quarantined(
    prepared: R56PreparedMuxBatchV1,
    reason: R56QuarantineReasonV1,
    published_native_prefix: u8,
    cursor_before: u8,
) -> R56MuxPublishOutcomeV1 {
    R56MuxPublishOutcomeV1::Quarantined(R56QuarantinedMuxBatchV1 {
        prepared,
        reason,
        published_native_prefix,
        confirmed_publications: Vec::new(),
        indeterminate_publication: None,
        untouched_request_indices: Vec::new(),
        cursor_before,
        cursor_committed: false,
    })
}

fn canonical_request_indices(prepared: &R56PreparedMuxBatchV1) -> Vec<u16> {
    (0..prepared.tickets.len())
        .map(|index| index as u16)
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
pub enum R56MuxWaitObservationV1 {
    Timeout,
    Completed(Vec<R56MuxTicketV1>),
}

#[derive(Debug, Eq, PartialEq)]
pub enum R56MuxWaitOutcomeV1 {
    Pending(R56PublishedMuxBatchV1),
    Completed(Vec<R56MuxRequestV1>),
    Rejected(Box<R56ReleaseFailureV1>),
}

pub fn r56_wait_mux_batch_model_only(
    published: R56PublishedMuxBatchV1,
    observation: R56MuxWaitObservationV1,
) -> R56MuxWaitOutcomeV1 {
    match observation {
        R56MuxWaitObservationV1::Timeout => {
            if published.is_exact_model_only() {
                R56MuxWaitOutcomeV1::Pending(published)
            } else {
                R56MuxWaitOutcomeV1::Rejected(Box::new(R56ReleaseFailureV1 { published }))
            }
        }
        R56MuxWaitObservationV1::Completed(completed_tickets) => {
            match r56_release_completed_mux_batch_model_only(published, &completed_tickets) {
                Ok(requests) => R56MuxWaitOutcomeV1::Completed(requests),
                Err(failure) => R56MuxWaitOutcomeV1::Rejected(failure),
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R56ReleaseFailureV1 {
    published: R56PublishedMuxBatchV1,
}

impl R56ReleaseFailureV1 {
    pub fn request_identities_model_only(&self) -> Vec<R56RequestIdentityV1> {
        self.published.request_identities_model_only()
    }

    pub fn into_published_model_only(self) -> R56PublishedMuxBatchV1 {
        self.published
    }
}

pub fn r56_release_completed_mux_batch_model_only(
    published: R56PublishedMuxBatchV1,
    completed_tickets: &[R56MuxTicketV1],
) -> Result<Vec<R56MuxRequestV1>, Box<R56ReleaseFailureV1>> {
    if !published.is_exact_model_only() || completed_tickets != published.tickets_model_only() {
        return Err(Box::new(R56ReleaseFailureV1 { published }));
    }
    Ok(published.prepared.requests)
}

fn request_identities(requests: &[R56MuxRequestV1]) -> Vec<R56RequestIdentityV1> {
    requests
        .iter()
        .map(R56MuxRequestV1::identity_model_only)
        .collect()
}

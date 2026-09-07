//! Balanced multi-queue SDMA planning, publication, and aggregate completion.

use core::fmt;
use fe2o3_kfd_uapi::{KFD_GFX942_SDMA_ENGINE_COUNT_V1, KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1};
use std::time::Duration;

use super::{
    GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_V1, GFX942_SDMA_MAX_IN_FLIGHT_V1,
    GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1, GFX942_SDMA_MAX_STRIPED_QUEUES_V1,
    GFX942_SDMA_RING_SLOT_COUNT_V1, Gfx942SdmaCompletedCopyV1, Gfx942SdmaCopyRequestV1,
    Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1, Gfx942SdmaQueueOwnerV1, Gfx942SdmaQueueSetV1,
    Gfx942SdmaUnpublishedCopyRequestV1, PreparedSdmaBatchV1, PreparedSdmaPublicationFailureV1,
    map_multi_queue_plan_error, ticket_matches_queue_occurrence,
};
use crate::shared_memory::SharedGttMemorySessionV1;
mod tail_wait;
#[allow(unsafe_code)]
mod tail_wait_cpu;
pub(crate) use tail_wait::Gfx942SdmaStripedTailWaitOutcomeV1;

/// Closed spin-budget roster for the profiled striped-tail benchmark experiment.
///
/// This is a diagnostic policy selector, not a duration parser or a production
/// wait-policy input. The ordinary striped wait cannot receive this selector.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Gfx942SdmaStripedDiagnosticSpinBudgetV1 {
    /// Preserve the current 64-spin, 16-yield, then bounded-sleep policy.
    #[default]
    Current,
    /// Actively poll for 250 us before the existing adaptive tail.
    Micros250,
    /// Actively poll for 500 us before the existing adaptive tail.
    Micros500,
    /// Actively poll for 1 ms before the existing adaptive tail.
    Millis1,
    /// Actively poll for 1.5 ms before the existing adaptive tail.
    Micros1500,
    /// Actively poll for 3 ms before the existing adaptive tail.
    Millis3,
}

impl Gfx942SdmaStripedDiagnosticSpinBudgetV1 {
    /// Canonical command-line and record label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Micros250 => "250us",
            Self::Micros500 => "500us",
            Self::Millis1 => "1ms",
            Self::Micros1500 => "1500us",
            Self::Millis3 => "3ms",
        }
    }

    /// Configured elapsed active-spin floor in nanoseconds; zero means current policy.
    pub const fn nanoseconds(self) -> u64 {
        match self {
            Self::Current => 0,
            Self::Micros250 => 250_000,
            Self::Micros500 => 500_000,
            Self::Millis1 => 1_000_000,
            Self::Micros1500 => 1_500_000,
            Self::Millis3 => 3_000_000,
        }
    }

    /// Canonical wait-policy family recorded by the diagnostic benchmark.
    pub const fn policy_label(self) -> &'static str {
        match self {
            Self::Current => "current-adaptive-v1",
            Self::Micros250
            | Self::Micros500
            | Self::Millis1
            | Self::Micros1500
            | Self::Millis3 => "diagnostic-active-spin-floor-v1",
        }
    }

    pub(crate) const fn active_spin_floor(self) -> Option<Duration> {
        match self {
            Self::Current => None,
            _ => Some(Duration::from_nanos(self.nanoseconds())),
        }
    }
}

/// Availability and validity of one profiled tail-scan CPU-cost observation.
///
/// This status describes optional measurement data only. It is never a queue
/// completion result, a custody disposition, or an admission input.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Gfx942SdmaStripedWaitCpuMeasurementStatusV1 {
    /// One or both Linux thread observations were unavailable.
    #[default]
    Unavailable,
    /// An observed clock, counter, or delta was outside the admitted numeric domain.
    Invalid,
    /// All three deltas were observed and validated.
    Available,
}

/// Host-side decomposition of one successful profiled striped-tail wait.
///
/// These counters, monotonic durations, and best-effort Linux thread-cost
/// deltas are diagnostic observations. They are not GPU timestamps,
/// physical-engine counters, completion evidence, or an admission input.
/// Collecting them adds host timestamp, CPU-clock, and rusage observations to
/// the explicitly profiled path; the ordinary wait does not collect them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Gfx942SdmaStripedWaitDiagnosticsV1 {
    pub(crate) diagnostic_spin_budget: Gfx942SdmaStripedDiagnosticSpinBudgetV1,
    pub(crate) active_queue_count: u8,
    pub(crate) request_count: u16,
    pub(crate) tail_scan_rounds: u64,
    pub(crate) tail_observations: u64,
    pub(crate) spin_pauses: u64,
    pub(crate) yield_pauses: u64,
    pub(crate) sleep_pauses: u64,
    pub(crate) requested_sleep_ns: u64,
    pub(crate) first_tail_ready_ns: Option<u64>,
    pub(crate) all_tails_ready_ns: Option<u64>,
    pub(crate) bind_ns: u64,
    pub(crate) opening_currentness_ns: u64,
    pub(crate) tail_scan_ns: u64,
    pub(crate) final_audit_ns: u64,
    pub(crate) closing_currentness_ns: u64,
    pub(crate) retirement_ns: u64,
    pub(crate) tail_scan_cpu_measurement_status: Gfx942SdmaStripedWaitCpuMeasurementStatusV1,
    pub(crate) tail_scan_thread_cpu_ns: Option<u64>,
    pub(crate) tail_scan_voluntary_context_switches: Option<u64>,
    pub(crate) tail_scan_involuntary_context_switches: Option<u64>,
}

impl Gfx942SdmaStripedWaitDiagnosticsV1 {
    /// Closed wait-policy choice used by this profiled diagnostic wait.
    pub const fn diagnostic_spin_budget(self) -> Gfx942SdmaStripedDiagnosticSpinBudgetV1 {
        self.diagnostic_spin_budget
    }

    pub const fn active_queue_count(self) -> u8 {
        self.active_queue_count
    }

    pub const fn request_count(self) -> u16 {
        self.request_count
    }

    pub const fn tail_scan_rounds(self) -> u64 {
        self.tail_scan_rounds
    }

    pub const fn tail_observations(self) -> u64 {
        self.tail_observations
    }

    pub const fn spin_pauses(self) -> u64 {
        self.spin_pauses
    }

    pub const fn yield_pauses(self) -> u64 {
        self.yield_pauses
    }

    pub const fn sleep_pauses(self) -> u64 {
        self.sleep_pauses
    }

    /// Sum of requested sleep durations, not measured scheduler sleep time.
    pub const fn requested_sleep_ns(self) -> u64 {
        self.requested_sleep_ns
    }

    /// Host-monotonic offset after the first scan round that observed any ready tail.
    pub const fn first_tail_ready_ns(self) -> Option<u64> {
        self.first_tail_ready_ns
    }

    /// Host-monotonic offset after the first scan round that observed all tails ready.
    pub const fn all_tails_ready_ns(self) -> Option<u64> {
        self.all_tails_ready_ns
    }

    pub const fn bind_ns(self) -> u64 {
        self.bind_ns
    }

    pub const fn opening_currentness_ns(self) -> u64 {
        self.opening_currentness_ns
    }

    pub const fn tail_scan_ns(self) -> u64 {
        self.tail_scan_ns
    }

    pub const fn final_audit_ns(self) -> u64 {
        self.final_audit_ns
    }

    pub const fn closing_currentness_ns(self) -> u64 {
        self.closing_currentness_ns
    }

    pub const fn retirement_ns(self) -> u64 {
        self.retirement_ns
    }

    /// Availability and validity of the tail-scan CPU-cost fields.
    pub const fn tail_scan_cpu_measurement_status(
        self,
    ) -> Gfx942SdmaStripedWaitCpuMeasurementStatusV1 {
        self.tail_scan_cpu_measurement_status
    }

    /// Whether every tail-scan CPU-cost delta is available and valid.
    pub const fn tail_scan_cpu_measurement_available(self) -> bool {
        matches!(
            self.tail_scan_cpu_measurement_status,
            Gfx942SdmaStripedWaitCpuMeasurementStatusV1::Available
        )
    }

    /// Calling-thread CPU time consumed while the tail-scan/wait loop ran.
    pub const fn tail_scan_thread_cpu_ns(self) -> Option<u64> {
        self.tail_scan_thread_cpu_ns
    }

    /// Voluntary context-switch delta across the tail-scan/wait loop.
    pub const fn tail_scan_voluntary_context_switches(self) -> Option<u64> {
        self.tail_scan_voluntary_context_switches
    }

    /// Involuntary context-switch delta across the tail-scan/wait loop.
    pub const fn tail_scan_involuntary_context_switches(self) -> Option<u64> {
        self.tail_scan_involuntary_context_switches
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942SdmaMultiQueuePlanErrorV1 {
    QueueCount { actual: usize },
    DuplicateQueueId { queue_id: u32 },
    RequestCount { actual: usize, maximum: usize },
    InvalidCursor { actual: usize, queue_count: usize },
    Allocation,
}

impl fmt::Display for Gfx942SdmaMultiQueuePlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid gfx942 SDMA multi-queue plan: {self:?}")
    }
}

impl std::error::Error for Gfx942SdmaMultiQueuePlanErrorV1 {}

/// Deterministic bounded request-to-queue assignment for one striped submission.
///
/// This is a structural plan. It neither publishes packets nor proves queue currentness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaMultiQueuePlanV1 {
    queue_ids: Vec<u32>,
    first_queue: u16,
    request_count: u16,
    assignments: Vec<u16>,
    shard_counts: Vec<u16>,
}

impl Gfx942SdmaMultiQueuePlanV1 {
    pub fn new(
        queue_ids: &[u32],
        request_count: usize,
        first_queue: usize,
    ) -> Result<Self, Gfx942SdmaMultiQueuePlanErrorV1> {
        if queue_ids.len() < KFD_GFX942_SDMA_ENGINE_COUNT_V1 as usize
            || !queue_ids
                .len()
                .is_multiple_of(KFD_GFX942_SDMA_ENGINE_COUNT_V1 as usize)
            || queue_ids.len() > GFX942_SDMA_MAX_STRIPED_QUEUES_V1
        {
            return Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount {
                actual: queue_ids.len(),
            });
        }
        for (index, queue_id) in queue_ids.iter().copied().enumerate() {
            if queue_ids[..index].contains(&queue_id) {
                return Err(Gfx942SdmaMultiQueuePlanErrorV1::DuplicateQueueId { queue_id });
            }
        }
        let maximum = queue_ids
            .len()
            .checked_mul(GFX942_SDMA_MAX_IN_FLIGHT_V1)
            .ok_or(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount {
                actual: request_count,
                maximum: GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1,
            })?;
        if request_count == 0
            || request_count > maximum
            || request_count > GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1
        {
            return Err(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount {
                actual: request_count,
                maximum,
            });
        }
        if first_queue >= queue_ids.len() {
            return Err(Gfx942SdmaMultiQueuePlanErrorV1::InvalidCursor {
                actual: first_queue,
                queue_count: queue_ids.len(),
            });
        }
        let mut retained_queue_ids = Vec::new();
        retained_queue_ids
            .try_reserve_exact(queue_ids.len())
            .map_err(|_| Gfx942SdmaMultiQueuePlanErrorV1::Allocation)?;
        retained_queue_ids.extend_from_slice(queue_ids);
        let mut assignments = Vec::new();
        assignments
            .try_reserve_exact(request_count)
            .map_err(|_| Gfx942SdmaMultiQueuePlanErrorV1::Allocation)?;
        let mut shard_counts = Vec::new();
        shard_counts
            .try_reserve_exact(queue_ids.len())
            .map_err(|_| Gfx942SdmaMultiQueuePlanErrorV1::Allocation)?;
        shard_counts.resize(queue_ids.len(), 0_u16);
        for request_index in 0..request_count {
            let queue = (first_queue + request_index) % queue_ids.len();
            assignments.push(queue as u16);
            shard_counts[queue] += 1;
        }
        debug_assert!(
            shard_counts
                .iter()
                .all(|count| usize::from(*count) <= GFX942_SDMA_MAX_IN_FLIGHT_V1)
        );
        Ok(Self {
            queue_ids: retained_queue_ids,
            first_queue: first_queue as u16,
            request_count: request_count as u16,
            assignments,
            shard_counts,
        })
    }

    pub fn queue_ids(&self) -> &[u32] {
        &self.queue_ids
    }

    pub const fn first_queue(&self) -> usize {
        self.first_queue as usize
    }

    pub const fn request_count(&self) -> usize {
        self.request_count as usize
    }

    pub fn queue_for_request(&self, request_index: usize) -> Option<usize> {
        self.assignments
            .get(request_index)
            .map(|queue| *queue as usize)
    }

    pub fn shard_count(&self, queue: usize) -> Option<usize> {
        self.shard_counts.get(queue).map(|count| *count as usize)
    }

    pub fn active_shard_count(&self) -> usize {
        self.shard_counts
            .iter()
            .filter(|count| **count != 0)
            .count()
    }

    pub fn next_queue_after_success(&self) -> usize {
        (self.first_queue() + self.request_count()) % self.queue_ids.len()
    }

    pub fn is_current_for(&self, queue_ids: &[u32], first_queue: usize) -> bool {
        self.queue_ids.as_slice() == queue_ids && self.first_queue() == first_queue
    }

    pub fn is_balanced(&self) -> bool {
        let minimum = self.shard_counts.iter().copied().min().unwrap_or(0);
        let maximum = self.shard_counts.iter().copied().max().unwrap_or(0);
        maximum - minimum <= 1
    }
}

/// Exact custody record for one queue shard after successful or indeterminate publication.
/// A success result means the shard publication returned success; the `indeterminate` field of a
/// failure means mapped publication began but its final device-visible state is not known.
#[must_use = "published tickets retain queue-owned buffer custody until completion"]
pub struct Gfx942SdmaMultiQueueShardTicketsV1 {
    queue_ordinal: u16,
    queue_id: u32,
    request_indices: Vec<u16>,
    tickets: Vec<Gfx942SdmaCopyTicketV1>,
}

impl Gfx942SdmaMultiQueueShardTicketsV1 {
    pub const fn queue_ordinal(&self) -> usize {
        self.queue_ordinal as usize
    }

    pub const fn queue_id(&self) -> u32 {
        self.queue_id
    }

    pub fn request_indices(&self) -> &[u16] {
        &self.request_indices
    }

    pub const fn ticket_count(&self) -> usize {
        self.tickets.len()
    }

    pub(crate) fn tickets(&self) -> &[Gfx942SdmaCopyTicketV1] {
        &self.tickets
    }
}

impl fmt::Debug for Gfx942SdmaMultiQueueShardTicketsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942SdmaMultiQueueShardTicketsV1")
            .field("queue_ordinal", &self.queue_ordinal)
            .field("queue_id", &self.queue_id)
            .field("request_indices", &self.request_indices)
            .field("ticket_count", &self.tickets.len())
            .finish()
    }
}

/// Successful publication across every non-empty shard in one bounded plan.
#[must_use = "every shard contains live tickets that must be completed"]
pub struct Gfx942SdmaMultiQueueSubmissionV1 {
    plan: Gfx942SdmaMultiQueuePlanV1,
    shards: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
    completion: PreparedMultiQueueCompletionV1,
}

impl Gfx942SdmaMultiQueueSubmissionV1 {
    pub const fn plan(&self) -> &Gfx942SdmaMultiQueuePlanV1 {
        &self.plan
    }

    pub fn shards(&self) -> &[Gfx942SdmaMultiQueueShardTicketsV1] {
        &self.shards
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Gfx942SdmaMultiQueuePlanV1,
        Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
        PreparedMultiQueueCompletionV1,
    ) {
        (self.plan, self.shards, self.completion)
    }

    #[cfg(test)]
    pub(crate) fn exact_identity_for_unwind_test(&self) -> Gfx942SdmaMultiQueueIdentityForTestV1 {
        Gfx942SdmaMultiQueueIdentityForTestV1 {
            plan: self.plan.clone(),
            shards: self
                .shards
                .iter()
                .map(|shard| {
                    (
                        shard.queue_ordinal,
                        shard.queue_id,
                        shard.request_indices.clone(),
                        shard.tickets.clone(),
                    )
                })
                .collect(),
            ordered: self.completion.ordered.clone(),
            ordered_capacity: self.completion.ordered.capacity(),
            completed_capacity: self.completion.completed.capacity(),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Gfx942SdmaMultiQueueIdentityForTestV1 {
    plan: Gfx942SdmaMultiQueuePlanV1,
    shards: Vec<(u16, u32, Vec<u16>, Vec<Gfx942SdmaCopyTicketV1>)>,
    ordered: Vec<ValidatedMultiQueueCompletionEntryV1>,
    ordered_capacity: usize,
    completed_capacity: usize,
}

#[cfg(test)]
pub(crate) fn striped_submission_for_unwind_test() -> Gfx942SdmaMultiQueueSubmissionV1 {
    let queue_ids = [41_u32, 43];
    let plan =
        Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, 4, 0).expect("fixed unwind-test striped plan");
    let owner = fe2o3_runtime_model::QueueKeyV1 {
        vm: fe2o3_runtime_model::VmKeyV1 {
            device: fe2o3_runtime_model::DeviceKeyV1 {
                physical: fe2o3_runtime_model::PhysicalDeviceIdV1(7),
                generation: fe2o3_runtime_model::DeviceGenerationV1(11),
            },
            id: fe2o3_runtime_model::VmIdV1(13),
        },
        id: fe2o3_runtime_model::QueueInstanceIdV1(17),
        generation: fe2o3_runtime_model::QueueGenerationV1(19),
    };
    let mut shard_requests = [Vec::new(), Vec::new()];
    let mut shard_tickets = [Vec::new(), Vec::new()];
    let mut ordered = Vec::with_capacity(plan.request_count());
    for request_index in 0_u16..4 {
        let queue_ordinal = plan
            .queue_for_request(usize::from(request_index))
            .expect("fixed unwind-test request assignment");
        let ticket = Gfx942SdmaCopyTicketV1 {
            owner,
            queue_id: queue_ids[queue_ordinal],
            slot: request_index + 3,
            generation: u32::from(request_index) + 23,
        };
        shard_requests[queue_ordinal].push(request_index);
        shard_tickets[queue_ordinal].push(ticket);
        ordered.push(ValidatedMultiQueueCompletionEntryV1 {
            request_index,
            queue_ordinal: queue_ordinal as u16,
            ticket,
        });
    }
    Gfx942SdmaMultiQueueSubmissionV1 {
        plan,
        shards: queue_ids
            .into_iter()
            .enumerate()
            .map(
                |(queue_ordinal, queue_id)| Gfx942SdmaMultiQueueShardTicketsV1 {
                    queue_ordinal: queue_ordinal as u16,
                    queue_id,
                    request_indices: core::mem::take(&mut shard_requests[queue_ordinal]),
                    tickets: core::mem::take(&mut shard_tickets[queue_ordinal]),
                },
            )
            .collect(),
        completion: PreparedMultiQueueCompletionV1 {
            ordered,
            completed: Vec::with_capacity(4),
        },
    }
}

/// Completed custody for one striped submission in original request order.
#[must_use = "completed mapped-buffer custody must be retained or released"]
pub struct Gfx942SdmaMultiQueueCompletedV1 {
    plan: Gfx942SdmaMultiQueuePlanV1,
    completed: Vec<Gfx942SdmaCompletedCopyV1>,
}

impl Gfx942SdmaMultiQueueCompletedV1 {
    pub const fn plan(&self) -> &Gfx942SdmaMultiQueuePlanV1 {
        &self.plan
    }

    pub fn completed(&self) -> &[Gfx942SdmaCompletedCopyV1] {
        &self.completed
    }

    pub fn into_completed(self) -> Vec<Gfx942SdmaCompletedCopyV1> {
        self.completed
    }
}

/// One whole-submission striped completion observation.
#[must_use = "Pending retains every shard and Completed retains every mapped buffer"]
pub enum Gfx942SdmaMultiQueuePollV1 {
    Pending(Gfx942SdmaMultiQueueSubmissionV1),
    Completed(Gfx942SdmaMultiQueueCompletedV1),
}

struct IndexedSdmaRequestV1<R = Gfx942SdmaCopyRequestV1> {
    index: u16,
    request: R,
}

struct PreparedMultiQueueShardV1<P = PreparedSdmaBatchV1> {
    queue_ordinal: usize,
    request_indices: Vec<u16>,
    batch: P,
}

struct PreparedMultiQueueSdmaBatchV1<
    P = PreparedSdmaBatchV1,
    S = Gfx942SdmaMultiQueueShardTicketsV1,
    U = Gfx942SdmaUnpublishedCopyRequestV1,
> {
    plan: Gfx942SdmaMultiQueuePlanV1,
    shards: Vec<PreparedMultiQueueShardV1<P>>,
    preflight: MultiQueuePreflightStateV1,
    published_capacity: Vec<S>,
    unpublished_capacity: Vec<U>,
}

pub(crate) struct MultiQueueSdmaPreparationFailureV1<R = Gfx942SdmaCopyRequestV1> {
    pub(crate) error: Gfx942SdmaErrorV1,
    pub(crate) requests: Vec<R>,
}

pub(crate) struct MultiQueueSdmaPublicationFailureV1<
    S = Gfx942SdmaMultiQueueShardTicketsV1,
    U = Gfx942SdmaUnpublishedCopyRequestV1,
> {
    pub(crate) error: Gfx942SdmaErrorV1,
    pub(crate) plan: Gfx942SdmaMultiQueuePlanV1,
    pub(crate) published: Vec<S>,
    pub(crate) indeterminate: Option<S>,
    pub(crate) unpublished: Vec<U>,
}

pub(crate) enum MultiQueueSdmaSubmitFailureV1 {
    Preparation(MultiQueueSdmaPreparationFailureV1),
    Publication(MultiQueueSdmaPublicationFailureV1),
    PublishedValidation {
        error: Gfx942SdmaErrorV1,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ValidatedMultiQueueCompletionEntryV1 {
    request_index: u16,
    queue_ordinal: u16,
    ticket: Gfx942SdmaCopyTicketV1,
}

pub(crate) struct PreparedMultiQueueCompletionV1 {
    ordered: Vec<ValidatedMultiQueueCompletionEntryV1>,
    completed: Vec<Gfx942SdmaCompletedCopyV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MultiQueueCursorOutcomeV1 {
    CompleteSuccess,
    #[cfg(test)]
    Failure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MultiQueuePreflightStateV1 {
    expected_queue_mask: u16,
    prepared_queue_mask: u16,
    confirmed_queue_mask: u16,
    indeterminate_queue_mask: u16,
    publication_authorized: bool,
}

impl MultiQueuePreflightStateV1 {
    fn new(plan: &Gfx942SdmaMultiQueuePlanV1) -> Self {
        let mut expected_queue_mask = 0_u16;
        for queue in 0..plan.queue_ids().len() {
            if plan.shard_count(queue).is_some_and(|count| count != 0) {
                expected_queue_mask |= 1_u16 << queue;
            }
        }
        Self {
            expected_queue_mask,
            prepared_queue_mask: 0,
            confirmed_queue_mask: 0,
            indeterminate_queue_mask: 0,
            publication_authorized: false,
        }
    }

    fn record_prepared_queue(&mut self, queue: usize) -> Result<(), Gfx942SdmaErrorV1> {
        let Some(bit) = 1_u16.checked_shl(queue as u32) else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue preflight queue bound",
            ));
        };
        if self.publication_authorized
            || self.expected_queue_mask & bit == 0
            || self.prepared_queue_mask & bit != 0
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue duplicate or unexpected preflight",
            ));
        }
        self.prepared_queue_mask |= bit;
        Ok(())
    }

    fn authorize_publication(&mut self) -> Result<(), Gfx942SdmaErrorV1> {
        if self.publication_authorized
            || self.expected_queue_mask == 0
            || self.prepared_queue_mask != self.expected_queue_mask
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue publication before complete preflight",
            ));
        }
        self.publication_authorized = true;
        Ok(())
    }

    fn record_publication_observation(
        &mut self,
        queue: usize,
        observation: MultiQueuePublicationObservationV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Some(bit) = 1_u16.checked_shl(queue as u32) else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue publication queue bound",
            ));
        };
        if !self.publication_authorized
            || self.expected_queue_mask & bit == 0
            || (self.confirmed_queue_mask | self.indeterminate_queue_mask) & bit != 0
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue duplicate or unexpected publication observation",
            ));
        }
        match observation {
            MultiQueuePublicationObservationV1::Confirmed => {
                self.confirmed_queue_mask |= bit;
            }
            MultiQueuePublicationObservationV1::RecoverableNoEffect => {}
            MultiQueuePublicationObservationV1::Indeterminate => {
                if self.indeterminate_queue_mask != 0 {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "multi-queue duplicate indeterminate publication observation",
                    ));
                }
                self.indeterminate_queue_mask = bit;
            }
        }
        Ok(())
    }

    const fn publication_is_complete(&self) -> bool {
        self.publication_authorized
            && self.confirmed_queue_mask == self.expected_queue_mask
            && self.indeterminate_queue_mask == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MultiQueuePublicationObservationV1 {
    Confirmed,
    RecoverableNoEffect,
    Indeterminate,
}

impl Gfx942SdmaQueueSetV1 {
    // Inline partial-publication custody is preallocated before the first publication.
    #[allow(clippy::result_large_err)]
    pub(crate) fn submit_striped_multi_queue_batch(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<Gfx942SdmaMultiQueueSubmissionV1, MultiQueueSdmaSubmitFailureV1> {
        let (owners, next_owner) = match self {
            Self::Striped { owners, next_owner } => (owners, next_owner),
            Self::Generic(_) | Self::Directional(_) | Self::TerminalRetained { .. } => {
                return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                    MultiQueueSdmaPreparationFailureV1 {
                        error: Gfx942SdmaErrorV1::Contract(
                            "multi-queue submission requires a striped SDMA queue set",
                        ),
                        requests,
                    },
                ));
            }
        };
        let mut queue_ids = Vec::new();
        if queue_ids.try_reserve_exact(owners.len()).is_err() {
            return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                MultiQueueSdmaPreparationFailureV1 {
                    error: Gfx942SdmaErrorV1::Contract(
                        "multi-queue SDMA queue identity allocation",
                    ),
                    requests,
                },
            ));
        }
        queue_ids.extend(owners.iter().map(|owner| owner.queue_id));
        let plan = match Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, requests.len(), *next_owner) {
            Ok(plan) => plan,
            Err(error) => {
                return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                    MultiQueueSdmaPreparationFailureV1 {
                        error: map_multi_queue_plan_error(error),
                        requests,
                    },
                ));
            }
        };
        let mut completion = PreparedMultiQueueCompletionV1 {
            ordered: Vec::new(),
            completed: Vec::new(),
        };
        if completion
            .ordered
            .try_reserve_exact(plan.request_count())
            .is_err()
            || completion
                .completed
                .try_reserve_exact(plan.request_count())
                .is_err()
        {
            return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                MultiQueueSdmaPreparationFailureV1 {
                    error: Gfx942SdmaErrorV1::Contract("multi-queue completion custody allocation"),
                    requests,
                },
            ));
        }
        let prepared = match prepare_multi_queue_batch(
            owners.len(),
            plan,
            requests,
            |queue, requests| owners[queue].prepare_batch_recoverable(memory, requests),
            PreparedSdmaBatchV1::into_requests,
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                return Err(MultiQueueSdmaSubmitFailureV1::Preparation(failure));
            }
        };
        populate_prepared_striped_completion_roster(owners, &prepared, &mut completion);
        let published = publish_multi_queue_batch(
            prepared,
            |queue, batch| {
                let queue_id = batch.queue_id;
                match owners[queue].submit_prepared_batch_with_custody(memory, batch) {
                    Ok(tickets) => {
                        debug_assert!(tickets.iter().all(|ticket| ticket.queue_id == queue_id));
                        Ok((queue_id, tickets))
                    }
                    Err(failure) => Err((queue_id, failure)),
                }
            },
            PreparedSdmaBatchV1::into_requests,
            |queue_ordinal, queue_id, request_indices, tickets| {
                Gfx942SdmaMultiQueueShardTicketsV1 {
                    queue_ordinal: queue_ordinal as u16,
                    queue_id,
                    request_indices,
                    tickets,
                }
            },
            |request_index, request| Gfx942SdmaUnpublishedCopyRequestV1 {
                request_index,
                request,
            },
            |request| request.request_index,
            |plan, shards| Gfx942SdmaMultiQueueSubmissionV1 {
                plan,
                shards,
                completion,
            },
        );
        match published {
            Ok(submission) => {
                if let Err(error) = self.confirm_striped_multi_queue_completion(&submission) {
                    return Err(MultiQueueSdmaSubmitFailureV1::PublishedValidation {
                        error,
                        submission,
                    });
                }
                Ok(submission)
            }
            // Failure deliberately performs no cursor write.
            Err(failure) => Err(MultiQueueSdmaSubmitFailureV1::Publication(failure)),
        }
    }

    pub(crate) fn commit_striped_multi_queue_success(
        &mut self,
        plan: &Gfx942SdmaMultiQueuePlanV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Self::Striped { owners, next_owner } = self else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue cursor commit requires striped SDMA queues",
            ));
        };
        if plan.first_queue() != *next_owner
            || plan.queue_ids().len() != owners.len()
            || !plan
                .queue_ids()
                .iter()
                .copied()
                .eq(owners.iter().map(|owner| owner.queue_id))
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "stale multi-queue cursor commit",
            ));
        }
        *next_owner = cursor_after_multi_queue_outcome(
            *next_owner,
            plan,
            MultiQueueCursorOutcomeV1::CompleteSuccess,
        )?;
        Ok(())
    }

    fn confirm_striped_multi_queue_completion(
        &self,
        submission: &Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Self::Striped { owners, .. } = self else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped completion requires striped SDMA queues",
            ));
        };
        if submission.plan.queue_ids().len() != owners.len()
            || !submission
                .plan
                .queue_ids()
                .iter()
                .copied()
                .eq(owners.iter().map(|owner| owner.queue_id))
            || submission.shards.len() != submission.plan.active_shard_count()
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped completion submission topology",
            ));
        }

        let request_count = submission.plan.request_count();
        if submission.completion.ordered.len() != request_count
            || !submission.completion.completed.is_empty()
            || submission.completion.ordered.capacity() < request_count
            || submission.completion.completed.capacity() < request_count
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped completion storage was not preallocated",
            ));
        }
        let mut seen_requests = [false; GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1];
        let mut seen_slots = [0_u64; GFX942_SDMA_MAX_STRIPED_QUEUES_V1];
        let mut seen_shards = 0_u16;

        for shard in &submission.shards {
            let queue_ordinal = shard.queue_ordinal();
            let Some(owner) = owners.get(queue_ordinal) else {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped completion queue ordinal",
                ));
            };
            let Some(shard_bit) = 1_u16.checked_shl(queue_ordinal as u32) else {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped completion shard bound",
                ));
            };
            if seen_shards & shard_bit != 0
                || shard.queue_id != owner.queue_id
                || shard.request_indices.len() != shard.tickets.len()
                || submission.plan.shard_count(queue_ordinal) != Some(shard.tickets.len())
            {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped completion shard identity",
                ));
            }
            seen_shards |= shard_bit;
            for (&request_index, &ticket) in shard.request_indices.iter().zip(shard.tickets.iter())
            {
                let request_index_usize = usize::from(request_index);
                if request_index_usize >= request_count
                    || seen_requests[request_index_usize]
                    || submission.plan.queue_for_request(request_index_usize) != Some(queue_ordinal)
                {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "striped completion request identity",
                    ));
                }
                let slot = owner.validate_ticket(ticket)?;
                let slot_bit = 1_u64 << slot;
                if seen_slots[queue_ordinal] & slot_bit != 0 {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "duplicate striped completion ticket",
                    ));
                }
                seen_requests[request_index_usize] = true;
                seen_slots[queue_ordinal] |= slot_bit;
                let expected = submission.completion.ordered.get(request_index_usize);
                if expected
                    != Some(&ValidatedMultiQueueCompletionEntryV1 {
                        request_index,
                        queue_ordinal: queue_ordinal as u16,
                        ticket,
                    })
                {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "published striped completion roster mismatch",
                    ));
                }
            }
        }
        if seen_requests[..request_count].iter().any(|seen| !seen) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "incomplete striped completion custody",
            ));
        }
        Ok(())
    }

    pub(crate) fn observe_prepared_striped_multi_queue_completion(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        submission: &Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Result<bool, Gfx942SdmaErrorV1> {
        let Self::Striped { owners, .. } = self else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped completion requires striped SDMA queues",
            ));
        };
        observe_entire_completion_roster(&submission.completion.ordered, |entry| {
            let owner = owners.get_mut(usize::from(entry.queue_ordinal)).ok_or(
                Gfx942SdmaErrorV1::Contract("striped completion owner disappeared"),
            )?;
            let slot = owner.validate_ticket(entry.ticket)?;
            owner.observe_validated_slot_in_current_scope(memory, slot)
        })
    }

    // Returning the complete submission preserves all-or-nothing custody.
    #[allow(clippy::result_large_err)]
    pub(crate) fn retire_prepared_striped_multi_queue_completion(
        &mut self,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Result<
        Gfx942SdmaMultiQueueCompletedV1,
        (Gfx942SdmaErrorV1, Gfx942SdmaMultiQueueSubmissionV1),
    > {
        let Self::Striped { owners, .. } = self else {
            return Err((
                Gfx942SdmaErrorV1::Contract("striped retirement requires striped SDMA queues"),
                submission,
            ));
        };
        // This complete pass makes the subsequent custody moves infallible:
        // neither the queue roster nor its records can change between passes.
        if !entire_validated_completion_roster_remains_present(
            &submission.completion.ordered,
            |entry| {
                owners
                    .get(usize::from(entry.queue_ordinal))
                    .is_some_and(|owner| {
                        owner
                            .validate_ticket(entry.ticket)
                            .is_ok_and(|slot| owner.validated_slot_remains_present(slot))
                    })
            },
        ) {
            return Err((
                Gfx942SdmaErrorV1::Contract("striped retirement record disappeared"),
                submission,
            ));
        }
        let (plan, _shards, mut completion) = submission.into_parts();
        for entry in completion.ordered {
            let Some(owner) = owners.get_mut(usize::from(entry.queue_ordinal)) else {
                // The immutable preflight above checked this exact owner index.
                std::process::abort();
            };
            completion
                .completed
                .push(owner.retire_validated_slot(usize::from(entry.ticket.slot)));
        }
        Ok(Gfx942SdmaMultiQueueCompletedV1 {
            plan,
            completed: completion.completed,
        })
    }
}

fn populate_prepared_striped_completion_roster(
    owners: &[Gfx942SdmaQueueOwnerV1],
    prepared: &PreparedMultiQueueSdmaBatchV1,
    completion: &mut PreparedMultiQueueCompletionV1,
) {
    let request_count = prepared.plan.request_count();
    if !completion.ordered.is_empty()
        || !completion.completed.is_empty()
        || completion.ordered.capacity() < request_count
        || completion.completed.capacity() < request_count
        || prepared.plan.queue_ids().len() != owners.len()
        || prepared.shards.len() != prepared.plan.active_shard_count()
    {
        std::process::abort();
    }
    let mut seen_requests = [false; GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1];
    let mut seen_slots = [0_u64; GFX942_SDMA_MAX_STRIPED_QUEUES_V1];
    let mut seen_shards = 0_u16;
    for shard in &prepared.shards {
        let queue_ordinal = shard.queue_ordinal;
        let Some(owner) = owners.get(queue_ordinal) else {
            std::process::abort();
        };
        let Some(shard_bit) = 1_u16.checked_shl(queue_ordinal as u32) else {
            std::process::abort();
        };
        if seen_shards & shard_bit != 0
            || shard.batch.queue_id != owner.queue_id
            || shard.request_indices.len() != shard.batch.tickets.len()
            || prepared.plan.shard_count(queue_ordinal) != Some(shard.batch.tickets.len())
        {
            std::process::abort();
        }
        seen_shards |= shard_bit;
        for (&request_index, &ticket) in
            shard.request_indices.iter().zip(shard.batch.tickets.iter())
        {
            let request_index_usize = usize::from(request_index);
            let slot = usize::from(ticket.slot);
            if request_index_usize >= request_count
                || slot >= GFX942_SDMA_RING_SLOT_COUNT_V1
                || seen_requests[request_index_usize]
                || seen_slots[queue_ordinal] & (1_u64 << slot) != 0
                || prepared.plan.queue_for_request(request_index_usize) != Some(queue_ordinal)
                || !ticket_matches_queue_occurrence(ticket, owner.owner, owner.queue_id)
            {
                std::process::abort();
            }
            seen_requests[request_index_usize] = true;
            seen_slots[queue_ordinal] |= 1_u64 << slot;
            completion
                .ordered
                .push(ValidatedMultiQueueCompletionEntryV1 {
                    request_index,
                    queue_ordinal: queue_ordinal as u16,
                    ticket,
                });
        }
    }
    if completion.ordered.len() != request_count
        || seen_requests[..request_count].iter().any(|seen| !seen)
        || !order_validated_completion_entries(&mut completion.ordered)
    {
        std::process::abort();
    }
}

struct MultiQueueShardBuildV1<R = Gfx942SdmaCopyRequestV1> {
    request_indices: Vec<u16>,
    requests: Vec<R>,
}

fn prepare_multi_queue_batch<R, P, S, U>(
    queue_count: usize,
    plan: Gfx942SdmaMultiQueuePlanV1,
    requests: Vec<R>,
    mut prepare_shard: impl FnMut(usize, Vec<R>) -> Result<P, (Gfx942SdmaErrorV1, Vec<R>)>,
    mut prepared_into_requests: impl FnMut(P) -> Vec<R>,
) -> Result<PreparedMultiQueueSdmaBatchV1<P, S, U>, MultiQueueSdmaPreparationFailureV1<R>> {
    let allocation_error = || Gfx942SdmaErrorV1::Contract("multi-queue SDMA custody allocation");
    let mut shards = Vec::new();
    if shards.try_reserve_exact(queue_count).is_err() {
        return Err(MultiQueueSdmaPreparationFailureV1 {
            error: allocation_error(),
            requests,
        });
    }
    for queue in 0..queue_count {
        let count = plan.shard_count(queue).unwrap_or(0);
        let mut request_indices = Vec::new();
        let mut queue_requests = Vec::new();
        if request_indices.try_reserve_exact(count).is_err()
            || queue_requests.try_reserve_exact(count).is_err()
        {
            return Err(MultiQueueSdmaPreparationFailureV1 {
                error: allocation_error(),
                requests,
            });
        }
        shards.push(MultiQueueShardBuildV1 {
            request_indices,
            requests: queue_requests,
        });
    }
    let mut prepared_shards = Vec::new();
    let mut recovery_pairs = Vec::new();
    let mut recovered_requests = Vec::new();
    let mut published_capacity = Vec::new();
    let mut unpublished_capacity = Vec::new();
    let mut preflight = MultiQueuePreflightStateV1::new(&plan);
    if prepared_shards
        .try_reserve_exact(plan.active_shard_count())
        .is_err()
        || recovery_pairs
            .try_reserve_exact(plan.request_count())
            .is_err()
        || recovered_requests
            .try_reserve_exact(plan.request_count())
            .is_err()
        || published_capacity
            .try_reserve_exact(plan.active_shard_count())
            .is_err()
        || unpublished_capacity
            .try_reserve_exact(plan.request_count())
            .is_err()
    {
        return Err(MultiQueueSdmaPreparationFailureV1 {
            error: allocation_error(),
            requests,
        });
    }
    for (index, request) in requests.into_iter().enumerate() {
        let queue = plan
            .queue_for_request(index)
            .expect("plan covers every bounded request");
        shards[queue].request_indices.push(index as u16);
        shards[queue].requests.push(request);
    }
    for step in 0..queue_count {
        let queue = (plan.first_queue() + step) % queue_count;
        if shards[queue].requests.is_empty() {
            continue;
        }
        let queue_requests = std::mem::take(&mut shards[queue].requests);
        match prepare_shard(queue, queue_requests) {
            Ok(batch) => {
                let prepared = PreparedMultiQueueShardV1 {
                    queue_ordinal: queue,
                    request_indices: std::mem::take(&mut shards[queue].request_indices),
                    batch,
                };
                if let Err(error) = preflight.record_prepared_queue(queue) {
                    prepared_shards.push(prepared);
                    append_prepared_requests(
                        &mut recovery_pairs,
                        prepared_shards,
                        &mut prepared_into_requests,
                    );
                    for shard in shards {
                        append_indexed_requests(
                            &mut recovery_pairs,
                            shard.request_indices,
                            shard.requests,
                        );
                    }
                    recovery_pairs.sort_unstable_by_key(|request| request.index);
                    recovered_requests
                        .extend(recovery_pairs.into_iter().map(|request| request.request));
                    return Err(MultiQueueSdmaPreparationFailureV1 {
                        error,
                        requests: recovered_requests,
                    });
                }
                prepared_shards.push(prepared);
            }
            Err((error, queue_requests)) => {
                append_prepared_requests(
                    &mut recovery_pairs,
                    prepared_shards,
                    &mut prepared_into_requests,
                );
                append_indexed_requests(
                    &mut recovery_pairs,
                    std::mem::take(&mut shards[queue].request_indices),
                    queue_requests,
                );
                for shard in shards {
                    append_indexed_requests(
                        &mut recovery_pairs,
                        shard.request_indices,
                        shard.requests,
                    );
                }
                recovery_pairs.sort_unstable_by_key(|request| request.index);
                recovered_requests
                    .extend(recovery_pairs.into_iter().map(|request| request.request));
                return Err(MultiQueueSdmaPreparationFailureV1 {
                    error,
                    requests: recovered_requests,
                });
            }
        }
    }
    Ok(PreparedMultiQueueSdmaBatchV1 {
        plan,
        shards: prepared_shards,
        preflight,
        published_capacity,
        unpublished_capacity,
    })
}

// Boxing this error would add an allocation after a prior shard may be device-visible.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
fn publish_multi_queue_batch<R, P, T, S, U, O>(
    prepared: PreparedMultiQueueSdmaBatchV1<P, S, U>,
    mut publish_shard: impl FnMut(
        usize,
        P,
    ) -> Result<
        (u32, Vec<T>),
        (u32, PreparedSdmaPublicationFailureV1<P, T>),
    >,
    mut prepared_into_requests: impl FnMut(P) -> Vec<R>,
    mut make_published: impl FnMut(usize, u32, Vec<u16>, Vec<T>) -> S,
    mut make_unpublished: impl FnMut(u16, R) -> U,
    unpublished_index: impl Fn(&U) -> u16,
    make_submission: impl FnOnce(Gfx942SdmaMultiQueuePlanV1, Vec<S>) -> O,
) -> Result<O, MultiQueueSdmaPublicationFailureV1<S, U>> {
    let PreparedMultiQueueSdmaBatchV1 {
        plan,
        shards,
        mut preflight,
        mut published_capacity,
        mut unpublished_capacity,
    } = prepared;
    if let Err(error) = preflight.authorize_publication() {
        for shard in shards {
            append_unpublished_requests(
                &mut unpublished_capacity,
                shard.request_indices,
                prepared_into_requests(shard.batch),
                &mut make_unpublished,
            );
        }
        unpublished_capacity.sort_unstable_by_key(&unpublished_index);
        return Err(MultiQueueSdmaPublicationFailureV1 {
            error,
            plan,
            published: published_capacity,
            indeterminate: None,
            unpublished: unpublished_capacity,
        });
    }
    let mut pending = shards.into_iter();
    while let Some(shard) = pending.next() {
        let queue_ordinal = shard.queue_ordinal;
        match publish_shard(queue_ordinal, shard.batch) {
            Ok((queue_id, tickets)) => {
                debug_assert_eq!(tickets.len(), shard.request_indices.len());
                if preflight
                    .record_publication_observation(
                        queue_ordinal,
                        MultiQueuePublicationObservationV1::Confirmed,
                    )
                    .is_err()
                {
                    std::process::abort();
                }
                published_capacity.push(make_published(
                    queue_ordinal,
                    queue_id,
                    shard.request_indices,
                    tickets,
                ));
            }
            Err((_, PreparedSdmaPublicationFailureV1::Recoverable { error, prepared })) => {
                if preflight
                    .record_publication_observation(
                        queue_ordinal,
                        MultiQueuePublicationObservationV1::RecoverableNoEffect,
                    )
                    .is_err()
                {
                    std::process::abort();
                }
                append_unpublished_requests(
                    &mut unpublished_capacity,
                    shard.request_indices,
                    prepared_into_requests(prepared),
                    &mut make_unpublished,
                );
                for pending_shard in pending {
                    append_unpublished_requests(
                        &mut unpublished_capacity,
                        pending_shard.request_indices,
                        prepared_into_requests(pending_shard.batch),
                        &mut make_unpublished,
                    );
                }
                unpublished_capacity.sort_unstable_by_key(&unpublished_index);
                return Err(MultiQueueSdmaPublicationFailureV1 {
                    error,
                    plan,
                    published: published_capacity,
                    indeterminate: None,
                    unpublished: unpublished_capacity,
                });
            }
            Err((queue_id, PreparedSdmaPublicationFailureV1::Retained { error, tickets })) => {
                debug_assert_eq!(tickets.len(), shard.request_indices.len());
                if preflight
                    .record_publication_observation(
                        queue_ordinal,
                        MultiQueuePublicationObservationV1::Indeterminate,
                    )
                    .is_err()
                {
                    std::process::abort();
                }
                let indeterminate = Some(make_published(
                    queue_ordinal,
                    queue_id,
                    shard.request_indices,
                    tickets,
                ));
                for pending_shard in pending {
                    append_unpublished_requests(
                        &mut unpublished_capacity,
                        pending_shard.request_indices,
                        prepared_into_requests(pending_shard.batch),
                        &mut make_unpublished,
                    );
                }
                unpublished_capacity.sort_unstable_by_key(&unpublished_index);
                return Err(MultiQueueSdmaPublicationFailureV1 {
                    error,
                    plan,
                    published: published_capacity,
                    indeterminate,
                    unpublished: unpublished_capacity,
                });
            }
        }
    }
    if !preflight.publication_is_complete() {
        std::process::abort();
    }
    Ok(make_submission(plan, published_capacity))
}

fn append_prepared_requests<R, P>(
    output: &mut Vec<IndexedSdmaRequestV1<R>>,
    shards: Vec<PreparedMultiQueueShardV1<P>>,
    prepared_into_requests: &mut impl FnMut(P) -> Vec<R>,
) {
    for shard in shards {
        append_indexed_requests(
            output,
            shard.request_indices,
            prepared_into_requests(shard.batch),
        );
    }
}

fn append_indexed_requests<R>(
    output: &mut Vec<IndexedSdmaRequestV1<R>>,
    indices: Vec<u16>,
    requests: Vec<R>,
) {
    debug_assert_eq!(indices.len(), requests.len());
    output.extend(
        indices
            .into_iter()
            .zip(requests)
            .map(|(index, request)| IndexedSdmaRequestV1 { index, request }),
    );
}

fn append_unpublished_requests<R, U>(
    output: &mut Vec<U>,
    indices: Vec<u16>,
    requests: Vec<R>,
    make_unpublished: &mut impl FnMut(u16, R) -> U,
) {
    debug_assert_eq!(indices.len(), requests.len());
    output.extend(
        indices
            .into_iter()
            .zip(requests)
            .map(|(request_index, request)| make_unpublished(request_index, request)),
    );
}

#[cfg(test)]
fn multi_queue_custody_is_exact<'a>(
    plan: &Gfx942SdmaMultiQueuePlanV1,
    shards: impl IntoIterator<Item = (usize, &'a [u16])>,
    unpublished: impl IntoIterator<Item = usize>,
) -> bool {
    let mut seen = [false; GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1];
    let mut observed = 0usize;
    for (queue, indices) in shards {
        for index in indices.iter().map(|index| usize::from(*index)) {
            if index >= plan.request_count()
                || seen[index]
                || plan.queue_for_request(index) != Some(queue)
            {
                return false;
            }
            seen[index] = true;
            observed += 1;
        }
    }
    for index in unpublished {
        if index >= plan.request_count() || seen[index] {
            return false;
        }
        seen[index] = true;
        observed += 1;
    }
    observed == plan.request_count() && seen[..plan.request_count()].iter().all(|value| *value)
}

fn observe_entire_completion_roster<T, E>(
    entries: &[T],
    mut observe: impl FnMut(&T) -> Result<bool, E>,
) -> Result<bool, E> {
    let mut all_ready = true;
    for entry in entries {
        all_ready &= observe(entry)?;
    }
    Ok(all_ready)
}

fn entire_validated_completion_roster_remains_present(
    entries: &[ValidatedMultiQueueCompletionEntryV1],
    mut remains_present: impl FnMut(&ValidatedMultiQueueCompletionEntryV1) -> bool,
) -> bool {
    entries.iter().all(&mut remains_present)
}

fn order_validated_completion_entries(
    entries: &mut [ValidatedMultiQueueCompletionEntryV1],
) -> bool {
    entries.sort_unstable_by_key(|entry| entry.request_index);
    entries
        .iter()
        .enumerate()
        .all(|(index, entry)| usize::from(entry.request_index) == index)
}

pub(crate) const fn striped_sdma_queue_count_is_admitted(queue_count: u32) -> bool {
    queue_count >= KFD_GFX942_SDMA_ENGINE_COUNT_V1
        && queue_count.is_multiple_of(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
        && queue_count <= KFD_GFX942_SDMA_ENGINE_COUNT_V1 * KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1
}

pub(crate) const fn combined_striped_sdma_queue_count_is_admitted(queue_count: u32) -> bool {
    queue_count >= KFD_GFX942_SDMA_ENGINE_COUNT_V1
        && queue_count.is_multiple_of(KFD_GFX942_SDMA_ENGINE_COUNT_V1)
        && queue_count <= GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_V1 as u32
}

pub(super) fn next_striped_owner(
    current: usize,
    owner_count: usize,
) -> Result<usize, Gfx942SdmaErrorV1> {
    if owner_count == 0 || current >= owner_count {
        return Err(Gfx942SdmaErrorV1::Contract("striped SDMA owner cursor"));
    }
    Ok((current + 1) % owner_count)
}

fn cursor_after_multi_queue_outcome(
    current: usize,
    plan: &Gfx942SdmaMultiQueuePlanV1,
    outcome: MultiQueueCursorOutcomeV1,
) -> Result<usize, Gfx942SdmaErrorV1> {
    if plan.first_queue() != current {
        return Err(Gfx942SdmaErrorV1::Contract(
            "multi-queue cursor plan is stale",
        ));
    }
    Ok(match outcome {
        MultiQueueCursorOutcomeV1::CompleteSuccess => plan.next_queue_after_success(),
        #[cfg(test)]
        MultiQueueCursorOutcomeV1::Failure => current,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_runtime_model::{
        DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
        QueueKeyV1, VmIdV1, VmKeyV1,
    };

    #[test]
    fn diagnostic_spin_budget_is_a_closed_bounded_roster() {
        for (budget, label, nanoseconds) in [
            (
                Gfx942SdmaStripedDiagnosticSpinBudgetV1::Current,
                "current",
                0,
            ),
            (
                Gfx942SdmaStripedDiagnosticSpinBudgetV1::Micros250,
                "250us",
                250_000,
            ),
            (
                Gfx942SdmaStripedDiagnosticSpinBudgetV1::Micros500,
                "500us",
                500_000,
            ),
            (
                Gfx942SdmaStripedDiagnosticSpinBudgetV1::Millis1,
                "1ms",
                1_000_000,
            ),
            (
                Gfx942SdmaStripedDiagnosticSpinBudgetV1::Micros1500,
                "1500us",
                1_500_000,
            ),
            (
                Gfx942SdmaStripedDiagnosticSpinBudgetV1::Millis3,
                "3ms",
                3_000_000,
            ),
        ] {
            assert_eq!(budget.label(), label);
            assert_eq!(budget.nanoseconds(), nanoseconds);
            assert_eq!(
                budget.active_spin_floor(),
                (nanoseconds != 0).then(|| Duration::from_nanos(nanoseconds))
            );
        }
    }

    fn queue_key(physical: u64, queue: u64, generation: u64) -> QueueKeyV1 {
        QueueKeyV1 {
            vm: VmKeyV1 {
                device: DeviceKeyV1 {
                    physical: PhysicalDeviceIdV1(physical),
                    generation: DeviceGenerationV1(1),
                },
                id: VmIdV1(1),
            },
            id: QueueInstanceIdV1(queue),
            generation: QueueGenerationV1(generation),
        }
    }

    #[test]
    fn aggregate_observation_visits_every_entry_before_reporting_pending() {
        let entries = [0_usize, 1, 2, 3];
        for pending_index in 0..entries.len() {
            let mut visited = Vec::new();
            let ready = observe_entire_completion_roster(&entries, |entry| {
                visited.push(*entry);
                Ok::<bool, ()>(*entry != pending_index)
            })
            .unwrap();
            assert!(!ready);
            assert_eq!(visited, entries);
        }

        let mut visited = Vec::new();
        let ready = observe_entire_completion_roster(&entries, |entry| {
            visited.push(*entry);
            Ok::<bool, ()>(true)
        })
        .unwrap();
        assert!(ready);
        assert_eq!(visited, entries);
    }

    #[test]
    fn aggregate_observation_stops_only_for_terminal_observation_error() {
        let entries = [0_usize, 1, 2, 3];
        let mut visited = Vec::new();
        let result = observe_entire_completion_roster(&entries, |entry| {
            visited.push(*entry);
            if *entry == 2 {
                Err("injected unexpected completion")
            } else {
                Ok(*entry != 0)
            }
        });
        assert_eq!(result, Err("injected unexpected completion"));
        assert_eq!(visited, [0, 1, 2]);
    }

    #[test]
    fn aggregate_completion_reconstructs_original_request_order() {
        let ticket = |queue_id: u32, slot: u16| Gfx942SdmaCopyTicketV1 {
            owner: queue_key(7, 8, u64::from(queue_id)),
            queue_id,
            slot,
            generation: 1,
        };
        let mut entries = [
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 3,
                queue_ordinal: 1,
                ticket: ticket(11, 9),
            },
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 0,
                queue_ordinal: 0,
                ticket: ticket(10, 4),
            },
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 2,
                queue_ordinal: 0,
                ticket: ticket(10, 8),
            },
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 1,
                queue_ordinal: 1,
                ticket: ticket(11, 5),
            },
        ];
        assert!(order_validated_completion_entries(&mut entries));
        assert_eq!(
            entries.map(|entry| (entry.request_index, entry.queue_ordinal, entry.ticket.slot,)),
            [(0, 0, 4), (1, 1, 5), (2, 0, 8), (3, 1, 9)]
        );

        let mut duplicate = [
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 0,
                queue_ordinal: 0,
                ticket: ticket(10, 4),
            },
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 0,
                queue_ordinal: 1,
                ticket: ticket(11, 5),
            },
        ];
        assert!(!order_validated_completion_entries(&mut duplicate));

        let mut missing = [
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 0,
                queue_ordinal: 0,
                ticket: ticket(10, 4),
            },
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 2,
                queue_ordinal: 1,
                ticket: ticket(11, 5),
            },
        ];
        assert!(!order_validated_completion_entries(&mut missing));
    }

    #[test]
    fn aggregate_retirement_preflight_rejects_every_missing_record_without_prefix_move() {
        let ticket = |queue_id: u32, slot: u16| Gfx942SdmaCopyTicketV1 {
            owner: queue_key(7, 8, u64::from(queue_id)),
            queue_id,
            slot,
            generation: 1,
        };
        let entries = [
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 0,
                queue_ordinal: 0,
                ticket: ticket(10, 4),
            },
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 1,
                queue_ordinal: 1,
                ticket: ticket(11, 5),
            },
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 2,
                queue_ordinal: 0,
                ticket: ticket(10, 8),
            },
            ValidatedMultiQueueCompletionEntryV1 {
                request_index: 3,
                queue_ordinal: 1,
                ticket: ticket(11, 9),
            },
        ];
        for missing_request in 0..entries.len() {
            let admitted = entire_validated_completion_roster_remains_present(&entries, |entry| {
                usize::from(entry.request_index) != missing_request
            });
            let moved_count = if admitted { entries.len() } else { 0 };
            assert!(!admitted);
            assert_eq!(moved_count, 0);
        }
        assert!(entire_validated_completion_roster_remains_present(
            &entries,
            |_| true
        ));
    }

    #[test]
    fn aggregate_pending_observation_preserves_preallocated_full_ticket_roster() {
        let ticket = |request_index| Gfx942SdmaCopyTicketV1 {
            owner: queue_key(7, 8, 1),
            queue_id: 10,
            slot: request_index,
            generation: u32::from(request_index) + 1,
        };
        let mut completion = PreparedMultiQueueCompletionV1 {
            ordered: Vec::with_capacity(4),
            completed: Vec::with_capacity(4),
        };
        for request_index in 0_u16..4 {
            completion
                .ordered
                .push(ValidatedMultiQueueCompletionEntryV1 {
                    request_index,
                    queue_ordinal: 0,
                    ticket: ticket(request_index),
                });
        }
        let ordered_pointer = completion.ordered.as_ptr();
        let ordered_capacity = completion.ordered.capacity();
        let completed_pointer = completion.completed.as_ptr();
        let completed_capacity = completion.completed.capacity();
        let exact_roster = completion.ordered.clone();
        for pending_index in 0..completion.ordered.len() {
            assert!(
                !observe_entire_completion_roster(&completion.ordered, |entry| Ok::<bool, ()>(
                    usize::from(entry.request_index) != pending_index
                ),)
                .unwrap()
            );
            assert_eq!(completion.ordered, exact_roster);
            assert_eq!(completion.ordered.as_ptr(), ordered_pointer);
            assert_eq!(completion.ordered.capacity(), ordered_capacity);
            assert_eq!(completion.completed.as_ptr(), completed_pointer);
            assert_eq!(completion.completed.capacity(), completed_capacity);
            assert!(completion.completed.is_empty());
        }
    }

    #[test]
    fn successful_aggregate_api_does_not_expose_copyable_ticket_values() {
        let source = include_str!("multi_queue.rs")
            .split("#[cfg(test)]\nmod tests")
            .next()
            .unwrap();
        let shard_api = source
            .split("impl Gfx942SdmaMultiQueueShardTicketsV1 {")
            .nth(1)
            .unwrap()
            .split("impl fmt::Debug for Gfx942SdmaMultiQueueShardTicketsV1")
            .next()
            .unwrap();
        assert!(shard_api.contains("pub const fn ticket_count"));
        assert!(shard_api.contains("pub(crate) fn tickets"));
        assert!(!shard_api.contains("pub fn tickets"));
        assert!(!shard_api.contains("into_tickets"));

        let submission_api = source
            .split("impl Gfx942SdmaMultiQueueSubmissionV1 {")
            .nth(1)
            .unwrap()
            .split("pub struct Gfx942SdmaMultiQueueCompletedV1")
            .next()
            .unwrap();
        assert!(!submission_api.contains("into_shards"));

        let completion_entry = source
            .split("struct ValidatedMultiQueueCompletionEntryV1")
            .nth(1)
            .unwrap()
            .split("pub(crate) struct PreparedMultiQueueCompletionV1")
            .next()
            .unwrap();
        assert!(completion_entry.contains("ticket: Gfx942SdmaCopyTicketV1"));
        let observation = source
            .split("pub(crate) fn observe_prepared_striped_multi_queue_completion")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn retire_prepared_striped_multi_queue_completion")
            .next()
            .unwrap();
        assert!(observation.contains("owner.validate_ticket(entry.ticket)"));
    }

    #[test]
    fn striped_queue_cursor_is_deterministic_and_wraps() {
        assert_eq!(next_striped_owner(0, 4).unwrap(), 1);
        assert_eq!(next_striped_owner(2, 4).unwrap(), 3);
        assert_eq!(next_striped_owner(3, 4).unwrap(), 0);
        assert!(next_striped_owner(0, 0).is_err());
        assert!(next_striped_owner(4, 4).is_err());
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum InjectedMultiQueueFaultV1 {
        Preparation { call: usize },
        RecoverablePublication { call: usize },
        IndeterminatePublication { call: usize },
        ClosingCurrentness,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct InjectedMultiQueueOutcomeV1 {
        succeeded: bool,
        confirmed_queues: Vec<usize>,
        confirmed_requests: Vec<usize>,
        indeterminate_queue: Option<usize>,
        indeterminate_requests: Vec<usize>,
        untouched_requests: Vec<usize>,
        cursor: usize,
    }

    struct InjectedShardV1 {
        queue_ordinal: usize,
        queue_id: u32,
        request_indices: Vec<u16>,
        tickets: Vec<usize>,
    }

    struct InjectedSubmissionV1 {
        plan: Gfx942SdmaMultiQueuePlanV1,
        shards: Vec<InjectedShardV1>,
    }

    fn injected_multi_queue_outcome(
        plan: &Gfx942SdmaMultiQueuePlanV1,
        cursor: usize,
        fault: Option<InjectedMultiQueueFaultV1>,
    ) -> InjectedMultiQueueOutcomeV1 {
        fn shard_observations(shards: &[InjectedShardV1]) -> (Vec<usize>, Vec<usize>) {
            let queues = shards
                .iter()
                .map(|shard| {
                    assert_eq!(shard.queue_id, 10 + shard.queue_ordinal as u32);
                    assert_eq!(shard.request_indices.len(), shard.tickets.len());
                    assert_eq!(
                        shard
                            .request_indices
                            .iter()
                            .map(|index| usize::from(*index))
                            .collect::<Vec<_>>(),
                        shard.tickets
                    );
                    shard.queue_ordinal
                })
                .collect();
            let mut requests = shards
                .iter()
                .flat_map(|shard| shard.request_indices.iter().copied())
                .map(usize::from)
                .collect::<Vec<_>>();
            requests.sort_unstable();
            (queues, requests)
        }

        let requests = (0..plan.request_count()).collect::<Vec<_>>();
        let mut preparation_call = 0;
        let prepared: PreparedMultiQueueSdmaBatchV1<Vec<usize>, InjectedShardV1, (u16, usize)> =
            match prepare_multi_queue_batch(
                plan.queue_ids().len(),
                plan.clone(),
                requests,
                |_, requests| {
                    let this_call = preparation_call;
                    preparation_call += 1;
                    if fault == Some(InjectedMultiQueueFaultV1::Preparation { call: this_call }) {
                        Err((
                            Gfx942SdmaErrorV1::Contract("injected preparation failure"),
                            requests,
                        ))
                    } else {
                        Ok(requests)
                    }
                },
                |prepared| prepared,
            ) {
                Ok(prepared) => prepared,
                Err(failure) => {
                    assert!(matches!(
                        fault,
                        Some(InjectedMultiQueueFaultV1::Preparation { .. })
                    ));
                    return InjectedMultiQueueOutcomeV1 {
                        succeeded: false,
                        confirmed_queues: Vec::new(),
                        confirmed_requests: Vec::new(),
                        indeterminate_queue: None,
                        indeterminate_requests: Vec::new(),
                        untouched_requests: failure.requests,
                        cursor,
                    };
                }
            };
        assert_eq!(prepared.published_capacity.len(), 0);
        assert!(prepared.published_capacity.capacity() >= plan.active_shard_count());
        assert_eq!(prepared.unpublished_capacity.len(), 0);
        assert!(prepared.unpublished_capacity.capacity() >= plan.request_count());

        let mut publication_call = 0;
        let published = publish_multi_queue_batch(
            prepared,
            |queue, prepared| {
                let this_call = publication_call;
                publication_call += 1;
                let queue_id = plan.queue_ids()[queue];
                match fault {
                    Some(InjectedMultiQueueFaultV1::RecoverablePublication { call })
                        if call == this_call =>
                    {
                        Err((
                            queue_id,
                            PreparedSdmaPublicationFailureV1::Recoverable {
                                error: Gfx942SdmaErrorV1::Contract(
                                    "injected recoverable publication failure",
                                ),
                                prepared,
                            },
                        ))
                    }
                    Some(InjectedMultiQueueFaultV1::IndeterminatePublication { call })
                        if call == this_call =>
                    {
                        Err((
                            queue_id,
                            PreparedSdmaPublicationFailureV1::Retained {
                                error: Gfx942SdmaErrorV1::Contract(
                                    "injected indeterminate publication failure",
                                ),
                                tickets: prepared,
                            },
                        ))
                    }
                    _ => Ok((queue_id, prepared)),
                }
            },
            |prepared| prepared,
            |queue_ordinal, queue_id, request_indices, tickets| InjectedShardV1 {
                queue_ordinal,
                queue_id,
                request_indices,
                tickets,
            },
            |request_index, request| (request_index, request),
            |request| request.0,
            |plan, shards| InjectedSubmissionV1 { plan, shards },
        );
        match published {
            Ok(submission) => {
                assert_eq!(submission.plan, *plan);
                assert!(submission.shards.capacity() >= plan.active_shard_count());
                let (confirmed_queues, confirmed_requests) = shard_observations(&submission.shards);
                let succeeded = fault != Some(InjectedMultiQueueFaultV1::ClosingCurrentness);
                InjectedMultiQueueOutcomeV1 {
                    succeeded,
                    confirmed_queues,
                    confirmed_requests,
                    indeterminate_queue: None,
                    indeterminate_requests: Vec::new(),
                    untouched_requests: Vec::new(),
                    cursor: if succeeded {
                        cursor_after_multi_queue_outcome(
                            cursor,
                            plan,
                            MultiQueueCursorOutcomeV1::CompleteSuccess,
                        )
                        .unwrap()
                    } else {
                        cursor
                    },
                }
            }
            Err(failure) => {
                assert!(failure.published.capacity() >= plan.active_shard_count());
                assert!(failure.unpublished.capacity() >= plan.request_count());
                let (confirmed_queues, confirmed_requests) = shard_observations(&failure.published);
                let (indeterminate_queue, indeterminate_requests) = failure
                    .indeterminate
                    .as_ref()
                    .map(|shard| {
                        let (_, requests) = shard_observations(std::slice::from_ref(shard));
                        (Some(shard.queue_ordinal), requests)
                    })
                    .unwrap_or((None, Vec::new()));
                let untouched_requests = failure
                    .unpublished
                    .into_iter()
                    .map(|(index, request)| {
                        assert_eq!(usize::from(index), request);
                        request
                    })
                    .collect();
                InjectedMultiQueueOutcomeV1 {
                    succeeded: false,
                    confirmed_queues,
                    confirmed_requests,
                    indeterminate_queue,
                    indeterminate_requests,
                    untouched_requests,
                    cursor,
                }
            }
        }
    }

    #[test]
    fn multi_queue_plan_rejects_invalid_duplicate_and_overcapacity_inputs() {
        assert_eq!(
            Gfx942SdmaMultiQueuePlanV1::new(&[], 1, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { actual: 0 })
        );
        assert_eq!(
            Gfx942SdmaMultiQueuePlanV1::new(&[1, 2, 3], 1, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { actual: 3 })
        );
        let too_many_queues = (0..=GFX942_SDMA_MAX_STRIPED_QUEUES_V1 as u32).collect::<Vec<_>>();
        assert!(matches!(
            Gfx942SdmaMultiQueuePlanV1::new(&too_many_queues, 1, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { .. })
        ));
        assert_eq!(
            Gfx942SdmaMultiQueuePlanV1::new(&[7, 8, 7, 9], 1, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::DuplicateQueueId { queue_id: 7 })
        );
        assert!(matches!(
            Gfx942SdmaMultiQueuePlanV1::new(&[7, 8], 0, 0),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount { actual: 0, .. })
        ));
        assert!(matches!(
            Gfx942SdmaMultiQueuePlanV1::new(&[7, 8], 2 * GFX942_SDMA_MAX_IN_FLIGHT_V1 + 1, 0,),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount { .. })
        ));
        assert_eq!(
            Gfx942SdmaMultiQueuePlanV1::new(&[7, 8], 1, 2),
            Err(Gfx942SdmaMultiQueuePlanErrorV1::InvalidCursor {
                actual: 2,
                queue_count: 2,
            })
        );
    }

    #[test]
    fn multi_queue_plan_is_balanced_deterministic_fair_and_current() {
        let queue_ids = [10, 11, 12, 13];
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, 10, 2).unwrap();
        assert_eq!(
            (0..10)
                .map(|index| plan.queue_for_request(index).unwrap())
                .collect::<Vec<_>>(),
            [2, 3, 0, 1, 2, 3, 0, 1, 2, 3]
        );
        assert_eq!(
            (0..4)
                .map(|queue| plan.shard_count(queue).unwrap())
                .collect::<Vec<_>>(),
            [2, 2, 3, 3]
        );
        assert!(plan.is_balanced());
        assert_eq!(plan.active_shard_count(), 4);
        assert_eq!(plan.next_queue_after_success(), 0);
        assert!(plan.is_current_for(&queue_ids, 2));
        assert!(!plan.is_current_for(&[10, 12, 11, 13], 2));
        assert!(!plan.is_current_for(&queue_ids, 1));

        let mut cursor = 0;
        let mut selected = Vec::new();
        for _ in 0..8 {
            let single = Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, 1, cursor).unwrap();
            selected.push(single.queue_for_request(0).unwrap());
            cursor = single.next_queue_after_success();
        }
        assert_eq!(selected, [0, 1, 2, 3, 0, 1, 2, 3]);
    }

    #[test]
    fn multi_queue_cursor_advances_only_after_complete_success() {
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 3, 1).unwrap();
        assert_eq!(
            cursor_after_multi_queue_outcome(1, &plan, MultiQueueCursorOutcomeV1::CompleteSuccess,)
                .unwrap(),
            0
        );
        for failure_stage in ["preparation", "partial-publication", "terminal"] {
            assert_eq!(
                cursor_after_multi_queue_outcome(1, &plan, MultiQueueCursorOutcomeV1::Failure,)
                    .unwrap(),
                1,
                "{failure_stage} failure advanced the cursor",
            );
        }
        assert!(
            cursor_after_multi_queue_outcome(0, &plan, MultiQueueCursorOutcomeV1::CompleteSuccess,)
                .is_err()
        );
    }

    #[test]
    fn multi_queue_partial_progress_accounting_is_exact() {
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 10, 2).unwrap();
        let published = [(2, [0_u16, 4, 8].as_slice())];
        let indeterminate = [(3, [1_u16, 5, 9].as_slice())];
        let unpublished = [2_usize, 3, 6, 7];
        assert!(multi_queue_custody_is_exact(
            &plan,
            published.into_iter().chain(indeterminate),
            unpublished,
        ));
        assert!(!multi_queue_custody_is_exact(
            &plan,
            [(2, [0_u16, 4, 8].as_slice()), (3, [1_u16, 5, 9].as_slice())],
            [2_usize, 3, 6, 6],
        ));
        assert!(!multi_queue_custody_is_exact(
            &plan,
            [(1, [0_u16, 4, 8].as_slice()), (3, [1_u16, 5, 9].as_slice())],
            unpublished,
        ));
    }

    #[test]
    fn multi_queue_preflight_gate_rejects_hostile_ordering_without_publication() {
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 4, 2).unwrap();
        let mut preflight = MultiQueuePreflightStateV1::new(&plan);
        assert!(!preflight.publication_authorized);
        assert!(
            preflight
                .record_publication_observation(2, MultiQueuePublicationObservationV1::Confirmed)
                .is_err()
        );
        assert!(preflight.record_prepared_queue(4).is_err());
        assert!(preflight.record_prepared_queue(2).is_ok());
        assert!(preflight.record_prepared_queue(2).is_err());
        assert!(preflight.authorize_publication().is_err());
        assert!(!preflight.publication_authorized);
        assert!(preflight.record_prepared_queue(3).is_ok());
        assert!(preflight.record_prepared_queue(0).is_ok());
        assert!(preflight.record_prepared_queue(1).is_ok());
        assert!(preflight.authorize_publication().is_ok());
        assert!(preflight.publication_authorized);
        assert!(preflight.record_prepared_queue(0).is_err());
        assert!(preflight.authorize_publication().is_err());
        assert!(
            preflight
                .record_publication_observation(2, MultiQueuePublicationObservationV1::Confirmed)
                .is_ok()
        );
        assert!(
            preflight
                .record_publication_observation(2, MultiQueuePublicationObservationV1::Confirmed)
                .is_err()
        );
        assert!(!preflight.publication_is_complete());
    }

    #[test]
    fn multi_queue_injected_coordinator_reports_exact_custody_and_cursor_outcomes() {
        let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 10, 2).unwrap();

        assert_eq!(
            injected_multi_queue_outcome(&plan, 2, None),
            InjectedMultiQueueOutcomeV1 {
                succeeded: true,
                confirmed_queues: vec![2, 3, 0, 1],
                confirmed_requests: (0..10).collect(),
                indeterminate_queue: None,
                indeterminate_requests: vec![],
                untouched_requests: vec![],
                cursor: 0,
            }
        );
        assert_eq!(
            injected_multi_queue_outcome(
                &plan,
                2,
                Some(InjectedMultiQueueFaultV1::Preparation { call: 1 }),
            ),
            InjectedMultiQueueOutcomeV1 {
                succeeded: false,
                confirmed_queues: vec![],
                confirmed_requests: vec![],
                indeterminate_queue: None,
                indeterminate_requests: vec![],
                untouched_requests: (0..10).collect(),
                cursor: 2,
            }
        );
        assert_eq!(
            injected_multi_queue_outcome(
                &plan,
                2,
                Some(InjectedMultiQueueFaultV1::RecoverablePublication { call: 1 }),
            ),
            InjectedMultiQueueOutcomeV1 {
                succeeded: false,
                confirmed_queues: vec![2],
                confirmed_requests: vec![0, 4, 8],
                indeterminate_queue: None,
                indeterminate_requests: vec![],
                untouched_requests: vec![1, 2, 3, 5, 6, 7, 9],
                cursor: 2,
            }
        );
        assert_eq!(
            injected_multi_queue_outcome(
                &plan,
                2,
                Some(InjectedMultiQueueFaultV1::IndeterminatePublication { call: 1 }),
            ),
            InjectedMultiQueueOutcomeV1 {
                succeeded: false,
                confirmed_queues: vec![2],
                confirmed_requests: vec![0, 4, 8],
                indeterminate_queue: Some(3),
                indeterminate_requests: vec![1, 5, 9],
                untouched_requests: vec![2, 3, 6, 7],
                cursor: 2,
            }
        );
        assert_eq!(
            injected_multi_queue_outcome(
                &plan,
                2,
                Some(InjectedMultiQueueFaultV1::ClosingCurrentness),
            ),
            InjectedMultiQueueOutcomeV1 {
                succeeded: false,
                confirmed_queues: vec![2, 3, 0, 1],
                confirmed_requests: (0..10).collect(),
                indeterminate_queue: None,
                indeterminate_requests: vec![],
                untouched_requests: vec![],
                cursor: 2,
            }
        );
    }

    #[test]
    fn multi_queue_publication_requires_fully_prepared_private_custody() {
        let source = include_str!("multi_queue.rs");
        let coordinator = source
            .split("pub(crate) fn submit_striped_multi_queue_batch")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn directional_observation")
            .next()
            .unwrap();
        let prepare = coordinator.find("prepare_multi_queue_batch").unwrap();
        let publish = coordinator.find("publish_multi_queue_batch").unwrap();
        assert!(prepare < publish);

        let prepare_body = source
            .split("fn prepare_multi_queue_batch<")
            .nth(1)
            .unwrap()
            .split("fn publish_multi_queue_batch<")
            .next()
            .unwrap();
        assert!(coordinator.contains("prepare_batch_recoverable"));
        assert!(coordinator.contains("submit_prepared_batch_with_custody"));
        assert!(prepare_body.contains("prepare_shard(queue, queue_requests)"));
        assert!(!prepare_body.contains("publish_shard(queue_ordinal"));

        let publish_body = source
            .split("fn publish_multi_queue_batch<")
            .nth(1)
            .unwrap()
            .split("fn append_prepared_requests")
            .next()
            .unwrap();
        assert!(publish_body.contains("publish_shard(queue_ordinal, shard.batch)"));
        assert!(!publish_body.contains("try_reserve"));
        assert!(!publish_body.contains("Vec::new"));
        assert!(!publish_body.contains(".collect"));
        assert!(!publish_body.contains("to_string"));
    }
}

//! Experimental logical-lane mux over two persistent ordinary SDMA queues.

use core::fmt;

use fe2o3_runtime_model::QueueKeyV1;

use super::{
    Gfx942SdmaMultiQueueCompletedV1, Gfx942SdmaMultiQueuePlanV1,
    Gfx942SdmaMultiQueueShardTicketsV1, Gfx942SdmaMultiQueueSubmissionV1,
    MultiQueueSdmaPreparationFailureV1, MultiQueueSdmaPublicationFailureV1,
    MultiQueueSdmaSubmitFailureV1,
};
use crate::sdma::{
    GFX942_SDMA_MAX_IN_FLIGHT_V1, Gfx942SdmaCompletedCopyV1, Gfx942SdmaCopyRequestV1,
    Gfx942SdmaErrorV1, Gfx942SdmaQueueObservationV1, Gfx942SdmaQueueSetCreationFailureV1,
    Gfx942SdmaQueueSetV1, Gfx942SdmaUnpublishedCopyRequestV1,
};
use crate::shared_memory::SharedGttMemorySessionV1;

pub const GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2: usize = 2;
pub const GFX942_SDMA_LOGICAL_MUX_MAX_REQUESTS_V2: usize =
    GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2 * GFX942_SDMA_MAX_IN_FLIGHT_V1;
pub const GFX942_SDMA_LOGICAL_MUX_MAX_REQUESTS_PER_NATIVE_QUEUE_V2: usize =
    GFX942_SDMA_MAX_IN_FLIGHT_V1;

pub const fn gfx942_sdma_logical_mux_lane_count_is_admitted_v2(lane_count: u32) -> bool {
    matches!(lane_count, 2 | 4 | 8 | 14 | 16)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942SdmaLogicalMuxPlanErrorV2 {
    NativeQueueIdentity,
    LogicalLaneCount { actual: u32 },
    RequestCount { actual: usize },
    InvalidCursor { actual: usize, lane_count: usize },
}

impl fmt::Display for Gfx942SdmaLogicalMuxPlanErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid gfx942 SDMA logical-mux plan: {self:?}")
    }
}

impl std::error::Error for Gfx942SdmaLogicalMuxPlanErrorV2 {}

/// Exact deterministic mapping from logical lanes to two native queues.
///
/// Logical lanes are scheduling labels, not independently progressing HIP
/// streams. Requests are visited in original order from `first_logical_lane`;
/// each native shard is the stable filter of that order for `lane % 2`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaLogicalMuxPlanV2 {
    native_queue_ids: [u32; GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2],
    logical_lane_count: u8,
    first_logical_lane: u8,
    request_count: u8,
}

impl Gfx942SdmaLogicalMuxPlanV2 {
    pub fn new(
        native_queue_ids: [u32; GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2],
        logical_lane_count: u32,
        request_count: usize,
        first_logical_lane: usize,
    ) -> Result<Self, Gfx942SdmaLogicalMuxPlanErrorV2> {
        if native_queue_ids[0] == native_queue_ids[1] {
            return Err(Gfx942SdmaLogicalMuxPlanErrorV2::NativeQueueIdentity);
        }
        if !gfx942_sdma_logical_mux_lane_count_is_admitted_v2(logical_lane_count) {
            return Err(Gfx942SdmaLogicalMuxPlanErrorV2::LogicalLaneCount {
                actual: logical_lane_count,
            });
        }
        if !(GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2
            ..=GFX942_SDMA_LOGICAL_MUX_MAX_REQUESTS_V2)
            .contains(&request_count)
        {
            return Err(Gfx942SdmaLogicalMuxPlanErrorV2::RequestCount {
                actual: request_count,
            });
        }
        let lane_count = logical_lane_count as usize;
        if first_logical_lane >= lane_count {
            return Err(Gfx942SdmaLogicalMuxPlanErrorV2::InvalidCursor {
                actual: first_logical_lane,
                lane_count,
            });
        }
        let plan = Self {
            native_queue_ids,
            logical_lane_count: logical_lane_count as u8,
            first_logical_lane: first_logical_lane as u8,
            request_count: request_count as u8,
        };
        if (0..GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2).any(|native| {
            plan.native_shard_count(native).unwrap_or(usize::MAX) > GFX942_SDMA_MAX_IN_FLIGHT_V1
        }) {
            return Err(Gfx942SdmaLogicalMuxPlanErrorV2::RequestCount {
                actual: request_count,
            });
        }
        Ok(plan)
    }

    pub const fn native_queue_ids(&self) -> &[u32; GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2] {
        &self.native_queue_ids
    }

    pub const fn logical_lane_count(&self) -> usize {
        self.logical_lane_count as usize
    }

    pub const fn first_logical_lane(&self) -> usize {
        self.first_logical_lane as usize
    }

    pub const fn request_count(&self) -> usize {
        self.request_count as usize
    }

    pub fn logical_lane_for_request(&self, request_index: usize) -> Option<usize> {
        (request_index < self.request_count())
            .then(|| (self.first_logical_lane() + request_index) % self.logical_lane_count())
    }

    pub fn native_queue_for_request(&self, request_index: usize) -> Option<usize> {
        self.logical_lane_for_request(request_index)
            .map(|lane| lane % 2)
    }

    pub fn native_shard_count(&self, native_queue: usize) -> Option<usize> {
        if native_queue >= GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2 {
            return None;
        }
        Some(
            (0..self.request_count())
                .filter(|index| self.native_queue_for_request(*index) == Some(native_queue))
                .count(),
        )
    }

    pub fn next_logical_lane_after_success(&self) -> usize {
        (self.first_logical_lane() + self.request_count()) % self.logical_lane_count()
    }

    pub fn is_current_for(
        &self,
        native_queue_ids: &[u32; GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2],
        logical_lane_count: usize,
        first_logical_lane: usize,
    ) -> bool {
        &self.native_queue_ids == native_queue_ids
            && self.logical_lane_count() == logical_lane_count
            && self.first_logical_lane() == first_logical_lane
    }

    fn matches_native_plan(&self, plan: &Gfx942SdmaMultiQueuePlanV1) -> bool {
        plan.queue_ids() == self.native_queue_ids
            && plan.first_queue() == self.first_logical_lane() % 2
            && plan.request_count() == self.request_count()
            && plan.active_shard_count() == GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2
            && (0..self.request_count())
                .all(|index| plan.queue_for_request(index) == self.native_queue_for_request(index))
            && (0..GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2)
                .all(|native| plan.shard_count(native) == self.native_shard_count(native))
    }
}

/// Two native queues and the admitted logical-lane count they multiplex.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaLogicalMuxObservationV2 {
    logical_lane_count: u8,
    native_queues: [Gfx942SdmaQueueObservationV1; GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2],
}

impl Gfx942SdmaLogicalMuxObservationV2 {
    pub const fn logical_lane_count(self) -> usize {
        self.logical_lane_count as usize
    }

    pub const fn native_queues(
        self,
    ) -> [Gfx942SdmaQueueObservationV1; GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2] {
        self.native_queues
    }
}

/// Borrowed audit view of one published native shard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942SdmaLogicalMuxNativeShardObservationV2<'a> {
    native_queue: usize,
    queue_id: u32,
    request_indices: &'a [u16],
    retained_ticket_count: usize,
}

impl<'a> Gfx942SdmaLogicalMuxNativeShardObservationV2<'a> {
    pub const fn native_queue(self) -> usize {
        self.native_queue
    }

    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }

    pub const fn request_indices(self) -> &'a [u16] {
        self.request_indices
    }

    pub const fn retained_ticket_count(self) -> usize {
        self.retained_ticket_count
    }
}

/// Successful two-native publication with exact lower ticket custody.
#[must_use = "both native shards retain queue-owned buffers until completion"]
pub struct Gfx942SdmaLogicalMuxSubmissionV2 {
    plan: Gfx942SdmaLogicalMuxPlanV2,
    lower: Gfx942SdmaMultiQueueSubmissionV1,
}

impl Gfx942SdmaLogicalMuxSubmissionV2 {
    pub const fn plan(&self) -> &Gfx942SdmaLogicalMuxPlanV2 {
        &self.plan
    }

    pub fn native_shard_count(&self) -> usize {
        self.lower.shards().len()
    }

    pub fn native_shard(
        &self,
        index: usize,
    ) -> Option<Gfx942SdmaLogicalMuxNativeShardObservationV2<'_>> {
        logical_mux_shard_observation_v2(self.lower.shards().get(index)?)
    }

    pub(crate) const fn lower(&self) -> &Gfx942SdmaMultiQueueSubmissionV1 {
        &self.lower
    }

    pub(crate) fn into_parts(
        self,
    ) -> (Gfx942SdmaLogicalMuxPlanV2, Gfx942SdmaMultiQueueSubmissionV1) {
        (self.plan, self.lower)
    }

    pub(crate) fn from_parts(
        plan: Gfx942SdmaLogicalMuxPlanV2,
        lower: Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Self {
        if !plan.matches_native_plan(lower.plan()) {
            std::process::abort();
        }
        Self { plan, lower }
    }
}

/// Completed custody reconstructed in original request order.
#[must_use = "completed mapped-buffer custody must be retained or released"]
pub struct Gfx942SdmaLogicalMuxCompletedV2 {
    plan: Gfx942SdmaLogicalMuxPlanV2,
    lower: Gfx942SdmaMultiQueueCompletedV1,
}

impl Gfx942SdmaLogicalMuxCompletedV2 {
    pub const fn plan(&self) -> &Gfx942SdmaLogicalMuxPlanV2 {
        &self.plan
    }

    pub fn completed(&self) -> &[Gfx942SdmaCompletedCopyV1] {
        self.lower.completed()
    }

    pub fn into_completed(self) -> Vec<Gfx942SdmaCompletedCopyV1> {
        self.lower.into_completed()
    }
}

#[must_use = "Pending retains both native shards and Completed retains every mapped buffer"]
pub enum Gfx942SdmaLogicalMuxPollV2 {
    Pending(Gfx942SdmaLogicalMuxSubmissionV2),
    Completed(Gfx942SdmaLogicalMuxCompletedV2),
}

pub(crate) struct LogicalMuxSdmaPreparationFailureV2 {
    pub(crate) error: Gfx942SdmaErrorV1,
    pub(crate) requests: Vec<Gfx942SdmaCopyRequestV1>,
}

pub(crate) struct LogicalMuxSdmaPublicationFailureV2 {
    pub(crate) error: Gfx942SdmaErrorV1,
    pub(crate) plan: Gfx942SdmaLogicalMuxPlanV2,
    pub(crate) published: Vec<Gfx942SdmaMultiQueueShardTicketsV1>,
    pub(crate) indeterminate: Option<Gfx942SdmaMultiQueueShardTicketsV1>,
    pub(crate) unpublished: Vec<Gfx942SdmaUnpublishedCopyRequestV1>,
}

pub(crate) enum LogicalMuxSdmaSubmitFailureV2 {
    Preparation(LogicalMuxSdmaPreparationFailureV2),
    Publication(LogicalMuxSdmaPublicationFailureV2),
    PublishedValidation {
        error: Gfx942SdmaErrorV1,
        submission: Gfx942SdmaLogicalMuxSubmissionV2,
    },
}

fn logical_mux_shard_observation_v2(
    shard: &Gfx942SdmaMultiQueueShardTicketsV1,
) -> Option<Gfx942SdmaLogicalMuxNativeShardObservationV2<'_>> {
    (shard.queue_ordinal() < GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2).then_some(
        Gfx942SdmaLogicalMuxNativeShardObservationV2 {
            native_queue: shard.queue_ordinal(),
            queue_id: shard.queue_id(),
            request_indices: shard.request_indices(),
            retained_ticket_count: shard.ticket_count(),
        },
    )
}

fn map_plan_error(error: Gfx942SdmaLogicalMuxPlanErrorV2) -> Gfx942SdmaErrorV1 {
    match error {
        Gfx942SdmaLogicalMuxPlanErrorV2::RequestCount { actual: 0 | 1 } => {
            Gfx942SdmaErrorV1::QueueFull
        }
        Gfx942SdmaLogicalMuxPlanErrorV2::NativeQueueIdentity
        | Gfx942SdmaLogicalMuxPlanErrorV2::LogicalLaneCount { .. }
        | Gfx942SdmaLogicalMuxPlanErrorV2::RequestCount { .. }
        | Gfx942SdmaLogicalMuxPlanErrorV2::InvalidCursor { .. } => {
            Gfx942SdmaErrorV1::Contract("logical-mux SDMA plan")
        }
    }
}

impl Gfx942SdmaQueueSetV1 {
    #[allow(clippy::result_large_err)]
    pub(crate) fn create_logical_mux_v2(
        memory: &mut SharedGttMemorySessionV1,
        owner: QueueKeyV1,
        logical_lane_count: u32,
        reserved_queue_ids: &[u32],
    ) -> Result<(Self, Gfx942SdmaLogicalMuxObservationV2), Gfx942SdmaQueueSetCreationFailureV1>
    {
        if !gfx942_sdma_logical_mux_lane_count_is_admitted_v2(logical_lane_count) {
            return Err(super::super::retryable_sdma_queue_set_creation_failure(
                Gfx942SdmaErrorV1::Contract(
                    "logical-mux SDMA lane count must be one of 2,4,8,14,16",
                ),
            ));
        }
        let (created, observations) = Self::create_striped(
            memory,
            owner,
            GFX942_SDMA_LOGICAL_MUX_NATIVE_QUEUE_COUNT_V2 as u32,
            reserved_queue_ids,
        )?;
        let Self::Striped { owners, .. } = created else {
            std::process::abort();
        };
        let [first, second] = observations.as_slice() else {
            std::process::abort();
        };
        let observation = Gfx942SdmaLogicalMuxObservationV2 {
            logical_lane_count: logical_lane_count as u8,
            native_queues: [*first, *second],
        };
        Ok((
            Self::LogicalMuxV2 {
                owners,
                logical_lane_count: logical_lane_count as u8,
                next_logical_lane: 0,
            },
            observation,
        ))
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn submit_logical_mux_batch_v2(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<Gfx942SdmaLogicalMuxSubmissionV2, LogicalMuxSdmaSubmitFailureV2> {
        let (native_queue_ids, logical_lane_count, first_logical_lane) = match self {
            Self::LogicalMuxV2 {
                owners,
                logical_lane_count,
                next_logical_lane,
            } => {
                let [first, second] = owners.as_slice() else {
                    return Err(LogicalMuxSdmaSubmitFailureV2::Preparation(
                        LogicalMuxSdmaPreparationFailureV2 {
                            error: Gfx942SdmaErrorV1::Contract("logical-mux native queue roster"),
                            requests,
                        },
                    ));
                };
                (
                    [first.queue_id, second.queue_id],
                    usize::from(*logical_lane_count),
                    usize::from(*next_logical_lane),
                )
            }
            Self::Generic(_)
            | Self::Directional(_)
            | Self::Striped { .. }
            | Self::TerminalRetained { .. } => {
                return Err(LogicalMuxSdmaSubmitFailureV2::Preparation(
                    LogicalMuxSdmaPreparationFailureV2 {
                        error: Gfx942SdmaErrorV1::Contract(
                            "logical-mux submission requires the V2 queue set",
                        ),
                        requests,
                    },
                ));
            }
        };
        let plan = match Gfx942SdmaLogicalMuxPlanV2::new(
            native_queue_ids,
            logical_lane_count as u32,
            requests.len(),
            first_logical_lane,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                return Err(LogicalMuxSdmaSubmitFailureV2::Preparation(
                    LogicalMuxSdmaPreparationFailureV2 {
                        error: map_plan_error(error),
                        requests,
                    },
                ));
            }
        };
        match self.submit_striped_multi_queue_batch(memory, requests) {
            Ok(lower) => {
                if !plan.matches_native_plan(lower.plan()) {
                    std::process::abort();
                }
                Ok(Gfx942SdmaLogicalMuxSubmissionV2 { plan, lower })
            }
            Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                MultiQueueSdmaPreparationFailureV1 { error, requests },
            )) => Err(LogicalMuxSdmaSubmitFailureV2::Preparation(
                LogicalMuxSdmaPreparationFailureV2 { error, requests },
            )),
            Err(MultiQueueSdmaSubmitFailureV1::Publication(
                MultiQueueSdmaPublicationFailureV1 {
                    error,
                    plan: lower_plan,
                    published,
                    indeterminate,
                    unpublished,
                },
            )) => {
                if !plan.matches_native_plan(&lower_plan) {
                    std::process::abort();
                }
                Err(LogicalMuxSdmaSubmitFailureV2::Publication(
                    LogicalMuxSdmaPublicationFailureV2 {
                        error,
                        plan,
                        published,
                        indeterminate,
                        unpublished,
                    },
                ))
            }
            Err(MultiQueueSdmaSubmitFailureV1::PublishedValidation {
                error,
                submission: lower,
            }) => {
                if !plan.matches_native_plan(lower.plan()) {
                    std::process::abort();
                }
                Err(LogicalMuxSdmaSubmitFailureV2::PublishedValidation {
                    error,
                    submission: Gfx942SdmaLogicalMuxSubmissionV2 { plan, lower },
                })
            }
        }
    }

    pub(crate) fn commit_logical_mux_success_v2(
        &mut self,
        plan: &Gfx942SdmaLogicalMuxPlanV2,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Self::LogicalMuxV2 {
            owners,
            logical_lane_count,
            next_logical_lane,
        } = self
        else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "logical-mux cursor commit requires the V2 queue set",
            ));
        };
        let [first, second] = owners.as_slice() else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "logical-mux cursor native queue roster",
            ));
        };
        if !plan.is_current_for(
            &[first.queue_id, second.queue_id],
            usize::from(*logical_lane_count),
            usize::from(*next_logical_lane),
        ) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "stale logical-mux cursor commit",
            ));
        }
        *next_logical_lane = plan.next_logical_lane_after_success() as u8;
        Ok(())
    }

    pub(crate) fn observe_logical_mux_completion_v2(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        submission: &Gfx942SdmaLogicalMuxSubmissionV2,
    ) -> Result<bool, Gfx942SdmaErrorV1> {
        if !submission.plan.matches_native_plan(submission.lower.plan()) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "logical-mux completion plan binding",
            ));
        }
        self.observe_prepared_striped_multi_queue_completion(memory, submission.lower())
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn retire_logical_mux_completion_v2(
        &mut self,
        submission: Gfx942SdmaLogicalMuxSubmissionV2,
    ) -> Result<
        Gfx942SdmaLogicalMuxCompletedV2,
        (Gfx942SdmaErrorV1, Gfx942SdmaLogicalMuxSubmissionV2),
    > {
        let (plan, lower) = submission.into_parts();
        match self.retire_prepared_striped_multi_queue_completion(lower) {
            Ok(lower) => Ok(Gfx942SdmaLogicalMuxCompletedV2 { plan, lower }),
            Err((error, lower)) => Err((error, Gfx942SdmaLogicalMuxSubmissionV2 { plan, lower })),
        }
    }

    pub(crate) fn wrap_logical_mux_completed_v2(
        plan: Gfx942SdmaLogicalMuxPlanV2,
        lower: Gfx942SdmaMultiQueueCompletedV1,
    ) -> Gfx942SdmaLogicalMuxCompletedV2 {
        Gfx942SdmaLogicalMuxCompletedV2 { plan, lower }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct InjectedNativeShardV2 {
        native_queue: usize,
        queue_id: u32,
        request_indices: Vec<u16>,
        requests: Vec<usize>,
    }

    struct InjectedNativeSubmissionV2 {
        shards: Vec<InjectedNativeShardV2>,
    }

    #[allow(clippy::result_large_err)]
    fn injected_publication_v2(
        recoverable_failure_call: Option<usize>,
        indeterminate_failure_call: Option<usize>,
    ) -> Result<
        InjectedNativeSubmissionV2,
        MultiQueueSdmaPublicationFailureV1<InjectedNativeShardV2, (u16, usize)>,
    > {
        let logical = Gfx942SdmaLogicalMuxPlanV2::new([101, 103], 16, 112, 15).unwrap();
        let native = Gfx942SdmaMultiQueuePlanV1::new(&[101, 103], 112, 1).unwrap();
        assert!(logical.matches_native_plan(&native));
        let prepared = match super::super::prepare_multi_queue_batch(
            2,
            native,
            (0..112).collect(),
            |_, requests| Ok::<_, (Gfx942SdmaErrorV1, Vec<usize>)>(requests),
            |prepared| prepared,
        ) {
            Ok(prepared) => prepared,
            Err(_) => panic!("injected preparation is infallible"),
        };
        let mut publication_call = 0;
        super::super::publish_multi_queue_batch(
            prepared,
            |native_queue, requests| {
                let call = publication_call;
                publication_call += 1;
                let queue_id = [101, 103][native_queue];
                if recoverable_failure_call == Some(call) {
                    Err((
                        queue_id,
                        crate::sdma::PreparedSdmaPublicationFailureV1::Recoverable {
                            error: Gfx942SdmaErrorV1::Contract("injected recoverable failure"),
                            prepared: requests,
                        },
                    ))
                } else if indeterminate_failure_call == Some(call) {
                    Err((
                        queue_id,
                        crate::sdma::PreparedSdmaPublicationFailureV1::Retained {
                            error: Gfx942SdmaErrorV1::Contract("injected indeterminate failure"),
                            tickets: requests,
                        },
                    ))
                } else {
                    Ok((queue_id, requests))
                }
            },
            |prepared| prepared,
            |native_queue, queue_id, request_indices, requests| InjectedNativeShardV2 {
                native_queue,
                queue_id,
                request_indices,
                requests,
            },
            |request_index, request| (request_index, request),
            |request| request.0,
            |_, shards| InjectedNativeSubmissionV2 { shards },
        )
    }

    #[test]
    fn admitted_lane_roster_is_closed() {
        for admitted in [2, 4, 8, 14, 16] {
            assert!(gfx942_sdma_logical_mux_lane_count_is_admitted_v2(admitted));
        }
        for rejected in [0, 1, 3, 6, 12, 15, 17] {
            assert!(!gfx942_sdma_logical_mux_lane_count_is_admitted_v2(rejected));
        }
    }

    #[test]
    fn depth_112_mapping_is_stable_balanced_and_bounded_for_every_profile() {
        for lane_count in [2, 4, 8, 14, 16] {
            for cursor in 0..lane_count {
                let plan =
                    Gfx942SdmaLogicalMuxPlanV2::new([101, 103], lane_count as u32, 112, cursor)
                        .unwrap();
                assert_eq!(plan.native_shard_count(0), Some(56));
                assert_eq!(plan.native_shard_count(1), Some(56));
                for index in 0..112 {
                    let lane = (cursor + index) % lane_count;
                    assert_eq!(plan.logical_lane_for_request(index), Some(lane));
                    assert_eq!(plan.native_queue_for_request(index), Some(lane % 2));
                }
                let queue_ids = [101, 103];
                let native = Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, 112, cursor % 2).unwrap();
                assert!(plan.matches_native_plan(&native));
            }
        }
    }

    #[test]
    fn maximum_is_exactly_63_requests_per_native_queue() {
        let plan = Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 16, 126, 15).unwrap();
        assert_eq!(plan.native_shard_count(0), Some(63));
        assert_eq!(plan.native_shard_count(1), Some(63));
        assert!(matches!(
            Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 16, 127, 0),
            Err(Gfx942SdmaLogicalMuxPlanErrorV2::RequestCount { actual: 127 })
        ));
    }

    #[test]
    fn plan_rejects_identity_lane_count_empty_and_stale_cursor() {
        assert_eq!(
            Gfx942SdmaLogicalMuxPlanV2::new([7, 7], 2, 1, 0),
            Err(Gfx942SdmaLogicalMuxPlanErrorV2::NativeQueueIdentity)
        );
        assert_eq!(
            Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 6, 1, 0),
            Err(Gfx942SdmaLogicalMuxPlanErrorV2::LogicalLaneCount { actual: 6 })
        );
        for request_count in [0, 1] {
            assert_eq!(
                Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 2, request_count, 0),
                Err(Gfx942SdmaLogicalMuxPlanErrorV2::RequestCount {
                    actual: request_count,
                })
            );
        }
        assert_eq!(
            Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 8, 2, 8),
            Err(Gfx942SdmaLogicalMuxPlanErrorV2::InvalidCursor {
                actual: 8,
                lane_count: 8,
            })
        );
    }

    #[test]
    fn cursor_advances_in_logical_not_native_domain() {
        let plan = Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 14, 112, 13).unwrap();
        assert_eq!(plan.next_logical_lane_after_success(), 13);
        let plan = Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 14, 3, 13).unwrap();
        assert_eq!(plan.next_logical_lane_after_success(), 2);
        assert_eq!(plan.native_queue_for_request(0), Some(1));
        assert_eq!(plan.native_queue_for_request(1), Some(0));
        assert_eq!(plan.native_queue_for_request(2), Some(1));
    }

    #[test]
    fn injected_success_publishes_each_stable_native_filter_once() {
        let submission = match injected_publication_v2(None, None) {
            Ok(submission) => submission,
            Err(_) => panic!("injected publication is infallible"),
        };
        assert_eq!(submission.shards.len(), 2);
        assert_eq!(
            submission
                .shards
                .iter()
                .map(|shard| shard.native_queue)
                .collect::<Vec<_>>(),
            [1, 0]
        );
        for shard in submission.shards {
            let expected = (0..112)
                .filter(|index| (15 + index) % 2 == shard.native_queue)
                .collect::<Vec<_>>();
            assert_eq!(shard.requests, expected);
            assert_eq!(
                shard.request_indices,
                shard
                    .requests
                    .iter()
                    .map(|index| *index as u16)
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn injected_first_native_no_effect_failure_preserves_all_custody_in_request_order() {
        let failure = injected_publication_v2(Some(0), None)
            .err()
            .expect("first publication must fail");
        assert!(failure.published.is_empty());
        assert!(failure.indeterminate.is_none());
        assert_eq!(failure.unpublished.len(), 112);
        assert!(
            failure
                .unpublished
                .iter()
                .enumerate()
                .all(|(expected, (index, request))| {
                    usize::from(*index) == expected && *request == expected
                })
        );
    }

    #[test]
    fn injected_first_native_indeterminate_failure_retains_rotated_exact_custody() {
        let logical = Gfx942SdmaLogicalMuxPlanV2::new([101, 103], 16, 112, 15).unwrap();
        let failure = injected_publication_v2(None, Some(0))
            .err()
            .expect("first publication must be indeterminate");
        assert!(matches!(
            failure.error,
            Gfx942SdmaErrorV1::Contract("injected indeterminate failure")
        ));
        assert_eq!(failure.plan.first_queue(), 1);
        assert_eq!(failure.plan.request_count(), 112);
        assert!(failure.published.is_empty());

        let indeterminate = failure
            .indeterminate
            .as_ref()
            .expect("rotated first native shard retains custody");
        assert_eq!(indeterminate.native_queue, 1);
        assert_eq!(indeterminate.queue_id, 103);
        assert_eq!(indeterminate.request_indices.len(), 56);
        assert!(
            indeterminate
                .request_indices
                .iter()
                .zip(&indeterminate.requests)
                .enumerate()
                .all(|(ordinal, (index, request))| {
                    usize::from(*index) == ordinal * 2 && *request == ordinal * 2
                })
        );

        assert_eq!(failure.unpublished.len(), 56);
        assert!(
            failure
                .unpublished
                .iter()
                .enumerate()
                .all(|(ordinal, (index, request))| {
                    usize::from(*index) == ordinal * 2 + 1 && *request == ordinal * 2 + 1
                })
        );
        assert_eq!(logical.first_logical_lane(), 15);
        assert_eq!(
            indeterminate.requests.len() + failure.unpublished.len(),
            logical.request_count()
        );
    }

    #[test]
    fn injected_second_native_failures_preserve_exact_partial_custody() {
        for indeterminate in [false, true] {
            let failure =
                injected_publication_v2((!indeterminate).then_some(1), indeterminate.then_some(1))
                    .err()
                    .expect("second publication must fail");
            assert_eq!(failure.published.len(), 1);
            assert_eq!(failure.published[0].native_queue, 1);
            assert_eq!(failure.published[0].requests.len(), 56);
            if indeterminate {
                let shard = failure.indeterminate.as_ref().unwrap();
                assert_eq!(shard.native_queue, 0);
                assert_eq!(shard.requests.len(), 56);
                assert!(failure.unpublished.is_empty());
            } else {
                assert!(failure.indeterminate.is_none());
                assert_eq!(failure.unpublished.len(), 56);
                assert!(
                    failure
                        .unpublished
                        .iter()
                        .all(|(index, request)| usize::from(*index) == *request)
                );
            }
            let accounted = failure.published[0].requests.len()
                + failure
                    .indeterminate
                    .as_ref()
                    .map_or(0, |shard| shard.requests.len())
                + failure.unpublished.len();
            assert_eq!(accounted, 112);
        }
    }

    #[test]
    fn native_plan_requires_both_exact_engines_before_cursor_commit_is_possible() {
        assert!(matches!(
            Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 16, 1, 0),
            Err(Gfx942SdmaLogicalMuxPlanErrorV2::RequestCount { actual: 1 })
        ));

        let plan = Gfx942SdmaLogicalMuxPlanV2::new([7, 9], 16, 2, 0).unwrap();
        let mut missing_native = Gfx942SdmaMultiQueuePlanV1::new(&[7, 9], 2, 0).unwrap();
        missing_native.shard_counts[1] = 0;
        assert!(!plan.matches_native_plan(&missing_native));

        let substituted_engine = Gfx942SdmaMultiQueuePlanV1::new(&[7, 11], 2, 0).unwrap();
        assert!(!plan.matches_native_plan(&substituted_engine));
    }
}

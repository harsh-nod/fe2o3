//! Count-branded original receipts; no conversion between four and sixteen.

use super::*;
use fe2o3_kfd::*;

#[allow(
    clippy::large_enum_variant,
    reason = "retains the exact original count-branded receipt without a post-publication allocation"
)]
pub(super) enum Batch {
    Four(Gfx942NativeFillRegistryBatchV1),
    Sixteen(Gfx942NativeFillResidentRegistryBatchV1<16>),
}
#[allow(
    clippy::large_enum_variant,
    reason = "retains the exact original completion without a post-publication allocation"
)]
pub(super) enum Completed {
    Four(Gfx942NativeFillRegistryCompletedV1),
    Sixteen(Gfx942NativeFillResidentRegistryCompletedV1<16>),
}
pub(super) enum Poll {
    Pending(Batch),
    Ready(Completed),
}
pub(super) struct PollFailure {
    pub(super) refused: Option<Batch>,
    pub(super) error: ComputeAqlQueueSessionErrorV1,
}
pub(super) struct RecycleFailure {
    pub(super) retryable: Option<Completed>,
    pub(super) error: ComputeAqlQueueSessionErrorV1,
}
#[allow(
    clippy::large_enum_variant,
    reason = "common native session custody remains inline; no fallible owner boxing"
)]
pub(super) enum Session {
    Once(Gfx942NativeFillRegistrySessionV1),
    Repeat2(Gfx942NativeFillRegistryRepeat2SessionV1),
    Sixteen(Gfx942NativeFillResidentRegistrySessionV1<16>),
}

impl Session {
    pub(super) fn with_retained_device_v1<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Once(s) => s.with_retained_device_v1(observe),
            Self::Repeat2(s) => s.with_retained_device_v1(observe),
            Self::Sixteen(s) => s.with_retained_device_v1(observe),
        }
    }
    pub(super) fn submit(
        &mut self,
        recipe: usize,
    ) -> Result<Batch, Gfx942FixedDispatchSubmissionFailureV1> {
        match self {
            Self::Once(s) => s.submit(recipe).map(Batch::Four),
            Self::Repeat2(s) => s.submit(recipe).map(Batch::Four),
            Self::Sixteen(s) => s.submit(recipe).map(Batch::Sixteen),
        }
    }
    #[allow(
        clippy::result_large_err,
        reason = "returns the original count-branded receipt inline"
    )]
    pub(super) fn poll(&mut self, batch: Batch) -> Result<Poll, PollFailure> {
        match (self, batch) {
            (Self::Once(s), Batch::Four(batch)) => {
                s.poll(batch).map(map_poll4).map_err(map_poll_failure4)
            }
            (Self::Repeat2(s), Batch::Four(batch)) => {
                s.poll(batch).map(map_poll4).map_err(map_poll_failure4)
            }
            (Self::Sixteen(s), Batch::Sixteen(batch)) => s
                .poll(batch)
                .map(|poll| match poll {
                    Gfx942NativeFillResidentRegistryPollV1::Pending(original) => {
                        Poll::Pending(Batch::Sixteen(original))
                    }
                    Gfx942NativeFillResidentRegistryPollV1::Ready(original) => {
                        Poll::Ready(Completed::Sixteen(original))
                    }
                })
                .map_err(|failure| PollFailure {
                    refused: failure.refused.map(Batch::Sixteen),
                    error: failure.error,
                }),
            (_, original) => Err(PollFailure {
                refused: Some(original),
                error: ComputeAqlQueueSessionErrorV1::Contract("registry receipt count mismatch"),
            }),
        }
    }
    #[allow(
        clippy::result_large_err,
        reason = "retains the original count-branded completion on refusal"
    )]
    pub(super) fn recycle(
        &mut self,
        completed: Completed,
    ) -> Result<Gfx942CompletionRecycleObservationV1, RecycleFailure> {
        match (self, completed) {
            (Self::Once(s), Completed::Four(original)) => {
                s.recycle(original).map_err(map_recycle_failure4)
            }
            (Self::Repeat2(s), Completed::Four(original)) => {
                s.recycle(original).map_err(map_recycle_failure4)
            }
            (Self::Sixteen(s), Completed::Sixteen(original)) => {
                s.recycle(original).map_err(|failure| RecycleFailure {
                    retryable: failure.retryable.map(Completed::Sixteen),
                    error: failure.error,
                })
            }
            (_, original) => Err(RecycleFailure {
                retryable: Some(original),
                error: ComputeAqlQueueSessionErrorV1::Contract(
                    "registry completion count mismatch",
                ),
            }),
        }
    }
    pub(super) fn read_into(
        &mut self,
        recipe: usize,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Once(s) => s.read_into(recipe, destination),
            Self::Repeat2(s) => s.read_into(recipe, destination),
            Self::Sixteen(s) => s.read_into(recipe, destination),
        }
    }
    pub(super) fn destroy(
        &mut self,
    ) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Once(s) => s.destroy(),
            Self::Repeat2(s) => s.destroy(),
            Self::Sixteen(s) => s.destroy(),
        }
    }
}
fn map_poll4(poll: Gfx942NativeFillRegistryPollV1) -> Poll {
    match poll {
        Gfx942NativeFillRegistryPollV1::Pending(original) => Poll::Pending(Batch::Four(original)),
        Gfx942NativeFillRegistryPollV1::Ready(original) => Poll::Ready(Completed::Four(original)),
    }
}
fn map_poll_failure4(failure: Gfx942NativeFillRegistryPollFailureV1) -> PollFailure {
    PollFailure {
        refused: failure.refused.map(Batch::Four),
        error: failure.error,
    }
}
fn map_recycle_failure4(failure: Gfx942NativeFillRegistryRecycleFailureV1) -> RecycleFailure {
    RecycleFailure {
        retryable: failure.retryable.map(Completed::Four),
        error: failure.error,
    }
}

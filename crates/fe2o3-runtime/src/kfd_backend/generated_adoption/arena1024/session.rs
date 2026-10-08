//! Closed count/order profiles use the same original native machinery.

use super::super::selected_step::{SelectedPoll, SelectedPollFailure};
use super::*;
use fe2o3_kfd::{
    ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1 as Error,
    Gfx942FixedDispatchSubmissionFailureV1, Gfx942IndependentFillArena2048BatchV1,
    Gfx942IndependentFillArena2048CompletedV1, Gfx942IndependentFillArena2048PollV1,
    Gfx942IndependentFillArena2048SessionV1, Gfx942IndependentFillArenaSessionV1,
    Gfx942NativeFillArenaPollV1,
};

pub(super) enum SessionV1 {
    Ordered(Gfx942NativeFillArenaSessionV1),
    Independent(Gfx942IndependentFillArenaSessionV1),
    Independent2048(Gfx942IndependentFillArena2048SessionV1),
}

pub(super) enum BatchV1 {
    Original(Gfx942NativeFillArenaBatchV1),
    Independent2048(Gfx942IndependentFillArena2048BatchV1),
}

pub(super) enum CompletedV1 {
    Original(Gfx942NativeFillArenaCompletedV1),
    Independent2048(Gfx942IndependentFillArena2048CompletedV1),
}

impl SessionV1 {
    pub(super) fn profile(&self) -> GeneratedProfileV1 {
        match self {
            Self::Ordered(_) => GeneratedProfileV1::NativeFillArena1024,
            Self::Independent(_) => GeneratedProfileV1::IndependentFillArena1024,
            Self::Independent2048(_) => GeneratedProfileV1::IndependentFillArena2048,
        }
    }

    pub(super) fn with_retained_device_v1<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, Error> {
        match self {
            Self::Ordered(session) => session.with_retained_device_v1(observe),
            Self::Independent(session) => session.with_retained_device_v1(observe),
            Self::Independent2048(session) => session.with_retained_device_v1(observe),
        }
    }

    pub(super) fn submit(
        &mut self,
        recipe: usize,
    ) -> Result<BatchV1, Gfx942FixedDispatchSubmissionFailureV1> {
        match self {
            Self::Ordered(session) => session.submit(recipe).map(BatchV1::Original),
            Self::Independent(session) => session.submit(recipe).map(BatchV1::Original),
            Self::Independent2048(session) => session.submit(recipe).map(BatchV1::Independent2048),
        }
    }

    // Return every original receipt inline, including a family mismatch.
    #[allow(clippy::result_large_err)]
    pub(super) fn poll(
        &mut self,
        batch: BatchV1,
    ) -> Result<SelectedPoll<BatchV1, CompletedV1>, SelectedPollFailure<BatchV1>> {
        let result = match (self, batch) {
            (Self::Ordered(session), BatchV1::Original(batch)) => session.poll(batch),
            (Self::Independent(session), BatchV1::Original(batch)) => session.poll(batch),
            (Self::Independent2048(session), BatchV1::Independent2048(batch)) => {
                return session
                    .poll(batch)
                    .map(|poll| match poll {
                        Gfx942IndependentFillArena2048PollV1::Pending(batch) => {
                            SelectedPoll::Pending(BatchV1::Independent2048(batch))
                        }
                        Gfx942IndependentFillArena2048PollV1::Ready(done) => {
                            SelectedPoll::Ready(CompletedV1::Independent2048(done))
                        }
                    })
                    .map_err(|failure| SelectedPollFailure {
                        error: failure.error,
                        refused: failure.refused.map(BatchV1::Independent2048),
                    });
            }
            (_, original) => {
                return Err(SelectedPollFailure {
                    error: Error::Contract("arena original receipt capacity mismatch"),
                    refused: Some(original),
                });
            }
        };
        result
            .map(|poll| match poll {
                Gfx942NativeFillArenaPollV1::Pending(batch) => {
                    SelectedPoll::Pending(BatchV1::Original(batch))
                }
                Gfx942NativeFillArenaPollV1::Ready(done) => {
                    SelectedPoll::Ready(CompletedV1::Original(done))
                }
            })
            .map_err(|failure| SelectedPollFailure {
                error: failure.error,
                refused: failure.refused.map(BatchV1::Original),
            })
    }

    // Retry retains the exact original completion without allocating a wrapper.
    #[allow(clippy::result_large_err)]
    pub(super) fn recycle(
        &mut self,
        completed: CompletedV1,
    ) -> Result<(), (Error, Option<CompletedV1>)> {
        let result = match (self, completed) {
            (Self::Ordered(session), CompletedV1::Original(done)) => session.recycle(done),
            (Self::Independent(session), CompletedV1::Original(done)) => session.recycle(done),
            (Self::Independent2048(session), CompletedV1::Independent2048(done)) => {
                return session.recycle(done).map(|_| ()).map_err(|failure| {
                    (
                        failure.error,
                        failure.retryable.map(CompletedV1::Independent2048),
                    )
                });
            }
            (_, original) => {
                return Err((
                    Error::Contract("arena original completion capacity mismatch"),
                    Some(original),
                ));
            }
        };
        result
            .map(|_| ())
            .map_err(|failure| (failure.error, failure.retryable.map(CompletedV1::Original)))
    }

    pub(super) fn read_into(&mut self, recipe: usize, destination: &mut [u8]) -> Result<(), Error> {
        match self {
            Self::Ordered(session) => session.read_into(recipe, destination),
            Self::Independent(session) => session.read_into(recipe, destination),
            Self::Independent2048(session) => session.read_into(recipe, destination),
        }
    }

    pub(super) fn destroy(&mut self) -> Result<ComputeAqlQueueDestroyedV1, Error> {
        match self {
            Self::Ordered(session) => session.destroy(),
            Self::Independent(session) => session.destroy(),
            Self::Independent2048(session) => session.destroy(),
        }
    }
}

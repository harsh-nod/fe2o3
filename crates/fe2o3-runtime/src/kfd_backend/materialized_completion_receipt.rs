//! Exact native receipts remain rooted through consuming poll and recycle calls.

use super::{
    ComputeAqlQueueSessionErrorV1, Gfx942CompletedDispatchBatchV1,
    Gfx942CompletionRecycleObservationV1, Gfx942DispatchBatchV1, Gfx942DispatchBindingErrorV1,
    Gfx942DispatchPollV1, Gfx942FixedDispatchRecycleFailureV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MaterializedConsumeV1 {
    Poll,
    Recycle,
}

pub(super) enum MaterializedCompletionReceiptV1 {
    Published(Gfx942DispatchBatchV1<1>),
    Completed(Gfx942CompletedDispatchBatchV1<1>),
    Retired(Gfx942CompletionRecycleObservationV1),
    Consuming(MaterializedConsumeV1),
}

impl MaterializedCompletionReceiptV1 {
    /// Returns true only after storing the first completed receipt.
    pub(super) fn poll_ready(
        &mut self,
        poll: impl FnOnce(
            Gfx942DispatchBatchV1<1>,
        ) -> Result<Gfx942DispatchPollV1<1>, ComputeAqlQueueSessionErrorV1>,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        if !matches!(self, Self::Published(_)) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let Self::Published(batch) =
            std::mem::replace(self, Self::Consuming(MaterializedConsumeV1::Poll))
        else {
            unreachable!()
        };
        match poll(batch)? {
            Gfx942DispatchPollV1::Pending(batch) => {
                *self = Self::Published(batch);
                Ok(false)
            }
            Gfx942DispatchPollV1::Ready(completed) => {
                *self = Self::Completed(completed);
                Ok(true)
            }
        }
    }

    /// A returned completed receipt, not its diagnostic, authorizes retry.
    pub(super) fn recycle_retired(
        &mut self,
        recycle: impl FnOnce(
            Gfx942CompletedDispatchBatchV1<1>,
        ) -> Result<
            Gfx942CompletionRecycleObservationV1,
            Gfx942FixedDispatchRecycleFailureV1<1>,
        >,
    ) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        if !matches!(self, Self::Completed(_)) {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        let Self::Completed(completed) =
            std::mem::replace(self, Self::Consuming(MaterializedConsumeV1::Recycle))
        else {
            unreachable!()
        };
        match recycle(completed) {
            Ok(observation) => {
                *self = Self::Retired(observation);
                Ok(true)
            }
            Err(failure) => {
                let (error, returned) = failure.into_parts();
                if let Some(completed) = returned {
                    *self = Self::Completed(completed);
                    Ok(false)
                } else {
                    Err(error)
                }
            }
        }
    }
}

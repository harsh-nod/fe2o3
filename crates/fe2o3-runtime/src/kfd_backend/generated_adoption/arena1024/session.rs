//! Closed ordering profiles delegate to the same original native machinery.

use super::*;
use fe2o3_kfd::{
    ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1,
    Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchSubmissionFailureV1,
    Gfx942IndependentFillArenaSessionV1, Gfx942NativeFillArenaPollFailureV1,
    Gfx942NativeFillArenaPollV1, Gfx942NativeFillArenaRecycleFailureV1,
};

pub(super) enum SessionV1 {
    Ordered(Gfx942NativeFillArenaSessionV1),
    Independent(Gfx942IndependentFillArenaSessionV1),
}

impl SessionV1 {
    pub(super) fn profile(&self) -> GeneratedProfileV1 {
        match self {
            Self::Ordered(_) => GeneratedProfileV1::NativeFillArena1024,
            Self::Independent(_) => GeneratedProfileV1::IndependentFillArena1024,
        }
    }

    pub(super) fn with_retained_device_v1<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Ordered(session) => session.with_retained_device_v1(observe),
            Self::Independent(session) => session.with_retained_device_v1(observe),
        }
    }

    pub(super) fn submit(
        &mut self,
        recipe: usize,
    ) -> Result<Gfx942NativeFillArenaBatchV1, Gfx942FixedDispatchSubmissionFailureV1> {
        match self {
            Self::Ordered(session) => session.submit(recipe),
            Self::Independent(session) => session.submit(recipe),
        }
    }

    // The original receipt is returned inline; refusal performs no allocation.
    #[allow(clippy::result_large_err)]
    pub(super) fn poll(
        &mut self,
        batch: Gfx942NativeFillArenaBatchV1,
    ) -> Result<Gfx942NativeFillArenaPollV1, Gfx942NativeFillArenaPollFailureV1> {
        match self {
            Self::Ordered(session) => session.poll(batch),
            Self::Independent(session) => session.poll(batch),
        }
    }

    // Retry owns the actual completion, never a reconstructed receipt.
    #[allow(clippy::result_large_err)]
    pub(super) fn recycle(
        &mut self,
        completed: Gfx942NativeFillArenaCompletedV1,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942NativeFillArenaRecycleFailureV1> {
        match self {
            Self::Ordered(session) => session.recycle(completed),
            Self::Independent(session) => session.recycle(completed),
        }
    }

    pub(super) fn read_into(
        &mut self,
        recipe: usize,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Ordered(session) => session.read_into(recipe, destination),
            Self::Independent(session) => session.read_into(recipe, destination),
        }
    }

    pub(super) fn destroy(
        &mut self,
    ) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Ordered(session) => session.destroy(),
            Self::Independent(session) => session.destroy(),
        }
    }
}

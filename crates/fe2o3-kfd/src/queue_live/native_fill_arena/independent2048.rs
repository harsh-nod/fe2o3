//! Capacity-branded original receipts over the shared native owner machinery.

use super::*;
use crate::queue::dispatch_binding::{
    GFX942_INDEPENDENT_FILL_ARENA2048_SLOTS_V1, Gfx942IndependentFillArena2048InputsV1,
    Gfx942IndependentFillArena2048StorageV1,
};

/// One original publication in an independent2048 arena, not a 1024 receipt.
#[derive(Debug)]
#[must_use = "retain the exact publication until original completion"]
pub struct Gfx942IndependentFillArena2048BatchV1(Gfx942NativeFillArenaBatchV1);

/// Actual selected-member completion; original signal recycle is still required.
#[derive(Debug)]
#[must_use = "the exact completed publication must be recycled"]
pub struct Gfx942IndependentFillArena2048CompletedV1(Gfx942NativeFillArenaCompletedV1);

#[derive(Debug)]
pub enum Gfx942IndependentFillArena2048PollV1 {
    Pending(Gfx942IndependentFillArena2048BatchV1),
    Ready(Gfx942IndependentFillArena2048CompletedV1),
}

#[derive(Debug)]
pub struct Gfx942IndependentFillArena2048PollFailureV1 {
    pub error: ComputeAqlQueueSessionErrorV1,
    pub refused: Option<Gfx942IndependentFillArena2048BatchV1>,
}

#[derive(Debug)]
pub struct Gfx942IndependentFillArena2048RecycleFailureV1 {
    pub error: ComputeAqlQueueSessionErrorV1,
    pub retryable: Option<Gfx942IndependentFillArena2048CompletedV1>,
}

/// Exactly 2048 single-use independent disjoint-WO fill slots. The original
/// common queue, CODE, kernarg and DATA remain indivisible until real destruction.
/// No measured native depth, duration, physical overlap or completion order is
/// implied. Incomplete Drop has the same fail-stop behavior as the 1024 owner.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942IndependentFillArena2048BatchV1 as Large,
///                 Gfx942IndependentFillArenaSessionV1 as Original};
/// fn refuse(session: &mut Original, batch: Large) { let _ = session.poll(batch); }
/// ```
pub struct Gfx942IndependentFillArena2048SessionV1(Gfx942NativeFillArenaSessionV1);

impl SharedGttMemorySessionV1 {
    /// Roots the original VM and every original input before any fallible native
    /// preparation, using the same PrimaryQueueConstruction owner. The ring must
    /// hold all 2048 packet slots (at least 128 KiB); no global queue limit changes.
    pub fn create_compute_aql_queue_with_independent_fill_arena2048_v1(
        self,
        ring_bytes: u32,
        inputs: Gfx942IndependentFillArena2048InputsV1<'_>,
        storage: Gfx942IndependentFillArena2048StorageV1,
    ) -> Result<Gfx942IndependentFillArena2048SessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_native_fill_arena_with_order_v1::<GFX942_INDEPENDENT_FILL_ARENA2048_SLOTS_V1>(
            ring_bytes,
            inputs.0,
            storage.0,
            ArenaOrderV1::IndependentDisjointWriteOnly,
            ArenaCapacityV1::Independent2048,
        )
        .map(Gfx942IndependentFillArena2048SessionV1)
    }
}

impl Gfx942IndependentFillArena2048SessionV1 {
    pub fn with_retained_device_v1<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        self.0.with_retained_device_v1(observe)
    }

    pub fn submit(
        &mut self,
        recipe: usize,
    ) -> Result<Gfx942IndependentFillArena2048BatchV1, Gfx942FixedDispatchSubmissionFailureV1> {
        self.0
            .submit(recipe)
            .map(Gfx942IndependentFillArena2048BatchV1)
    }

    // Refusal preserves the original receipt inline; no second owner allocation.
    #[allow(clippy::result_large_err)]
    pub fn poll(
        &mut self,
        batch: Gfx942IndependentFillArena2048BatchV1,
    ) -> Result<Gfx942IndependentFillArena2048PollV1, Gfx942IndependentFillArena2048PollFailureV1>
    {
        self.0
            .poll(batch.0)
            .map(|value| match value {
                Gfx942NativeFillArenaPollV1::Pending(batch) => {
                    Gfx942IndependentFillArena2048PollV1::Pending(
                        Gfx942IndependentFillArena2048BatchV1(batch),
                    )
                }
                Gfx942NativeFillArenaPollV1::Ready(completed) => {
                    Gfx942IndependentFillArena2048PollV1::Ready(
                        Gfx942IndependentFillArena2048CompletedV1(completed),
                    )
                }
            })
            .map_err(|failure| Gfx942IndependentFillArena2048PollFailureV1 {
                error: failure.error,
                refused: failure.refused.map(Gfx942IndependentFillArena2048BatchV1),
            })
    }

    // The retryable exact original completion must not be boxed or discarded.
    #[allow(clippy::result_large_err)]
    pub fn recycle(
        &mut self,
        completed: Gfx942IndependentFillArena2048CompletedV1,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942IndependentFillArena2048RecycleFailureV1>
    {
        self.0.recycle(completed.0).map_err(|failure| {
            Gfx942IndependentFillArena2048RecycleFailureV1 {
                error: failure.error,
                retryable: failure
                    .retryable
                    .map(Gfx942IndependentFillArena2048CompletedV1),
            }
        })
    }

    pub fn read_into(
        &mut self,
        recipe: usize,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.0.read_into(recipe, destination)
    }

    pub fn destroy(&mut self) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        self.0.destroy()
    }
}

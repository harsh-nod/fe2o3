//! Distinct profile; all native submission and cleanup use the original owner.

use super::*;

/// An independently ordered, checked disjoint-WO fill arena. It has no public
/// conversion to the ordinary ordered session. Original recipe occurrences
/// still authenticate every receipt against this exact common queue owner.
/// Independent headers permit concurrency; they do not establish observed
/// overlap, durations or out-of-order execution on any device.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942IndependentFillArenaSessionV1 as Independent,
///                 Gfx942NativeFillArenaSessionV1 as Ordered};
/// fn refuse(session: Independent) -> Ordered { session }
/// ```
pub struct Gfx942IndependentFillArenaSessionV1(Gfx942NativeFillArenaSessionV1);

impl SharedGttMemorySessionV1 {
    /// Retains the separately admitted independent profile in the same original
    /// primary construction root. No default ordering policy is relaxed.
    pub fn create_compute_aql_queue_with_independent_fill_arena_v1(
        self,
        ring_bytes: u32,
        inputs: Gfx942IndependentFillArenaInputsV1<'_>,
        storage: Gfx942NativeFillArenaStorageV1,
    ) -> Result<Gfx942IndependentFillArenaSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_native_fill_arena_with_order_v1(
            ring_bytes,
            inputs.0,
            storage,
            ArenaOrderV1::IndependentDisjointWriteOnly,
        )
        .map(Gfx942IndependentFillArenaSessionV1)
    }
}

impl Gfx942IndependentFillArenaSessionV1 {
    pub fn with_retained_device_v1<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        self.0.with_retained_device_v1(observe)
    }

    pub fn submit(
        &mut self,
        recipe: usize,
    ) -> Result<Gfx942NativeFillArenaBatchV1, Gfx942FixedDispatchSubmissionFailureV1> {
        self.0.submit(recipe)
    }

    // Refusal retains the original receipt inline, without another allocation.
    #[allow(clippy::result_large_err)]
    pub fn poll(
        &mut self,
        batch: Gfx942NativeFillArenaBatchV1,
    ) -> Result<Gfx942NativeFillArenaPollV1, Gfx942NativeFillArenaPollFailureV1> {
        self.0.poll(batch)
    }

    // Pinned completion returns its original retryable owner without boxing.
    #[allow(clippy::result_large_err)]
    pub fn recycle(
        &mut self,
        completed: Gfx942NativeFillArenaCompletedV1,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942NativeFillArenaRecycleFailureV1> {
        self.0.recycle(completed)
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

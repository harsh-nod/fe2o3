//! Explicit bounded reuse of the same original queue and four DATA owners.

use super::*;
use crate::queue::dispatch_binding::Gfx942NativeFillRegistryRepeat2StorageV1;

/// Eight publications maximum, at most four outstanding, on the same queue.
/// Rearm retains common native backing and requires exact recycle and a full
/// copied output for every first-cycle recipe. It provides no completion order,
/// overlap, new source admission, or release of common DATA.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942NativeFillRegistrySessionV1,
///     Gfx942NativeFillRegistryRepeat2SessionV1};
/// fn one_shot(_: Gfx942NativeFillRegistrySessionV1) {}
/// fn reject(owner: Gfx942NativeFillRegistryRepeat2SessionV1) { one_shot(owner); }
/// ```
pub struct Gfx942NativeFillRegistryRepeat2SessionV1 {
    inner: Gfx942NativeFillRegistrySessionV1,
}

impl SharedGttMemorySessionV1 {
    pub fn create_compute_aql_queue_with_native_fill_registry_repeat2_v1(
        self,
        ring_bytes: u32,
        inputs: Gfx942NativeFillRegistryInputsV1<'_>,
        storage: Gfx942NativeFillRegistryRepeat2StorageV1,
    ) -> Result<Gfx942NativeFillRegistryRepeat2SessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_native_fill_registry_profile(ring_bytes, inputs, storage.inner, true)
            .map(|inner| Gfx942NativeFillRegistryRepeat2SessionV1 { inner })
    }
}

impl Gfx942NativeFillRegistryRepeat2SessionV1 {
    pub fn with_retained_device_v1<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        self.inner.with_retained_device_v1(observe)
    }

    pub fn submit(
        &mut self,
        recipe: usize,
    ) -> Result<Gfx942NativeFillRegistryBatchV1, Gfx942FixedDispatchSubmissionFailureV1> {
        self.inner.submit(recipe)
    }

    // Exact original receipts stay inline; refusal requires no allocation.
    #[allow(clippy::result_large_err)]
    pub fn poll(
        &mut self,
        batch: Gfx942NativeFillRegistryBatchV1,
    ) -> Result<Gfx942NativeFillRegistryPollV1, Gfx942NativeFillRegistryPollFailureV1> {
        self.inner.poll(batch)
    }

    #[allow(clippy::result_large_err)]
    pub fn recycle(
        &mut self,
        completed: Gfx942NativeFillRegistryCompletedV1,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942NativeFillRegistryRecycleFailureV1>
    {
        self.inner.recycle(completed)
    }

    pub fn read_into(
        &mut self,
        recipe: usize,
        destination: &mut [u8],
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.inner.read_into(recipe, destination)
    }

    /// Advances all four original recipes once. No packet is published here.
    /// Validation is atomic before mutation and bracketed by actual original
    /// queue currentness. A failed closing probe terminalizes the original root.
    pub fn rearm_second_cycle(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let queue = self
            .inner
            .queue
            .as_mut()
            .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?;
        let storage = &mut self.inner.storage;
        queue.with_retained_device_v1(|_| storage.rearm_second_cycle())??;
        Ok(())
    }

    pub fn destroy(&mut self) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        self.inner.destroy()
    }
}

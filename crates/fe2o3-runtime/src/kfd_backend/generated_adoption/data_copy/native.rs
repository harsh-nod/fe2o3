use super::root::{Operations, Owners, Poll};
use fe2o3_kfd::{
    ComputeAqlQueueSessionV1, Gfx942DetachedFixedDispatchV1, Gfx942DetachedSdmaFailureV1,
    Gfx942DirectionalPersistentSdmaCompletedV1, Gfx942DirectionalPersistentSdmaCopyPollV1,
    Gfx942DirectionalPersistentSdmaExecutionFailureV1,
    Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
    Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
    Gfx942DirectionalPersistentSdmaSubmissionV1, Gfx942DirectionalQueuePersistentAllocationV1,
    Gfx942PersistentDependencyFrontierV1, Gfx942PersistentSdmaDirectionV1,
    Gfx942SdmaBufferTransitionFailureV1, Gfx942SdmaBufferV1, Gfx942SdmaDispatchDataBridgeV1,
};

pub(super) struct NativeOwners;

impl Owners for NativeOwners {
    type Source = (
        Gfx942DetachedFixedDispatchV1,
        Gfx942SdmaDispatchDataBridgeV1,
    );
    type Destination = Gfx942DirectionalQueuePersistentAllocationV1;
    type Buffer = Gfx942SdmaBufferV1;
    type Submission = Gfx942DirectionalPersistentSdmaSubmissionV1;
    type Completed = Gfx942DirectionalPersistentSdmaCompletedV1;
    type Frontier = Gfx942PersistentDependencyFrontierV1;
    type TransferFailure = Gfx942DetachedSdmaFailureV1;
    type SubmitFailure = Gfx942DirectionalPersistentSdmaSubmissionFailureV1;
    type PollFailure = Gfx942DirectionalPersistentSdmaExecutionFailureV1;
    type RetireFailure = Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1;
    type ReleaseFailure = Gfx942SdmaBufferTransitionFailureV1;
}

pub(super) struct NativeOperations<'a> {
    pub(super) queue: &'a mut ComputeAqlQueueSessionV1,
    pub(super) destination_offset: u64,
    pub(super) bytes: u32,
}

impl Operations<NativeOwners> for NativeOperations<'_> {
    fn transfer(
        &mut self,
        (detached, bridge): <NativeOwners as Owners>::Source,
    ) -> Result<Gfx942SdmaBufferV1, Gfx942DetachedSdmaFailureV1> {
        self.queue
            .transfer_detached_fixed_dispatch_to_sdma_v1(detached, bridge)
    }

    fn submit(
        &mut self,
        buffer: Gfx942SdmaBufferV1,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Result<
        Gfx942DirectionalPersistentSdmaSubmissionV1,
        Gfx942DirectionalPersistentSdmaSubmissionFailureV1,
    > {
        self.queue.submit_directional_persistent_sdma_copy_v1(
            destination,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            buffer,
            0,
            self.destination_offset,
            self.bytes,
        )
    }

    fn poll(
        &mut self,
        submission: Gfx942DirectionalPersistentSdmaSubmissionV1,
    ) -> Result<
        Poll<
            Gfx942DirectionalPersistentSdmaSubmissionV1,
            Gfx942DirectionalPersistentSdmaCompletedV1,
        >,
        Gfx942DirectionalPersistentSdmaExecutionFailureV1,
    > {
        self.queue
            .poll_directional_persistent_sdma_copy_v1(submission)
            .map(|poll| match poll {
                Gfx942DirectionalPersistentSdmaCopyPollV1::Pending(owner) => Poll::Pending(owner),
                Gfx942DirectionalPersistentSdmaCopyPollV1::Completed(owner) => {
                    Poll::Completed(owner)
                }
            })
    }

    fn split_completed(
        completed: Gfx942DirectionalPersistentSdmaCompletedV1,
    ) -> (
        Gfx942SdmaBufferV1,
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942PersistentDependencyFrontierV1,
    ) {
        let (destination, buffer, frontier) = completed.into_parts();
        (buffer, destination, frontier)
    }

    fn retire(
        &mut self,
        destination: Gfx942DirectionalQueuePersistentAllocationV1,
        frontier: Gfx942PersistentDependencyFrontierV1,
    ) -> Result<Gfx942DirectionalQueuePersistentAllocationV1, <NativeOwners as Owners>::RetireFailure>
    {
        destination.retire_settled_frontier_v1(frontier)
    }

    fn release(
        &mut self,
        buffer: Gfx942SdmaBufferV1,
    ) -> Result<(), Gfx942SdmaBufferTransitionFailureV1> {
        // This is physical release, not a pool recycle or historical completion.
        self.queue.release_sdma_buffer(buffer)
    }
}

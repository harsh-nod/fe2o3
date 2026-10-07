//! One owner's borrowed release mechanics, independent of queue-set policy.

#![forbid(unsafe_code)]

use super::*;
use crate::queue_linux::{LinuxDoorbellErrorV1, LinuxDoorbellReleaseProgressV1};
use crate::shared_memory::SdmaResourceCleanupCustodyV1;

pub(crate) trait SdmaOwnerReleaseMemoryV1 {
    fn sdma_release_currentness(&mut self) -> Result<(), MemorySessionError>;
    fn sdma_destroy(
        &mut self,
        args: &mut KfdIoctlDestroyQueueArgs,
    ) -> Result<(), rustix::io::Errno>;
    fn sdma_release_doorbell(
        &mut self,
        doorbell: &mut LinuxDoorbellSliceV1,
        progress: &mut LinuxDoorbellReleaseProgressV1,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        doorbell.release_retaining_v1(progress)
    }
    fn sdma_release_resources(
        &mut self,
        resources: &mut SdmaResourceCleanupCustodyV1,
    ) -> Result<(), MemorySessionError>;
}

impl SdmaOwnerReleaseMemoryV1 for SharedGttMemorySessionV1 {
    fn sdma_release_currentness(&mut self) -> Result<(), MemorySessionError> {
        self.check_queue_currentness()
    }
    fn sdma_destroy(
        &mut self,
        args: &mut KfdIoctlDestroyQueueArgs,
    ) -> Result<(), rustix::io::Errno> {
        destroy_queue(self.kfd_fd(), args)
    }
    fn sdma_release_resources(
        &mut self,
        resources: &mut SdmaResourceCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        self.release_queue_resources_in_place_v1(resources)
    }
}

#[derive(Default)]
pub(super) struct OwnerProgressV1 {
    pub(super) request: Option<KfdIoctlDestroyQueueArgs>,
    pub(super) attempted: bool,
    pub(super) result: Option<Result<(), rustix::io::Errno>>,
    pub(super) doorbell: LinuxDoorbellReleaseProgressV1,
    pub(super) resources: Option<SdmaResourceCleanupCustodyV1>,
}

impl OwnerProgressV1 {
    /// The caller roots both owner and progress, preflights once, and handles
    /// terminalization. This helper neither consumes authority nor catches panics.
    pub(super) fn destroy_in_place(
        &mut self,
        owner: &mut Gfx942SdmaQueueOwnerV1,
        memory: &mut impl SdmaOwnerReleaseMemoryV1,
        doorbell_error: impl FnOnce(LinuxDoorbellErrorV1) -> Gfx942SdmaErrorV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        memory.sdma_release_currentness()?;
        let original = KfdIoctlDestroyQueueArgs::new(owner.queue_id);
        self.request = Some(original);
        owner.poisoned = true;
        self.attempted = true;
        let result = memory.sdma_destroy(self.request.as_mut().expect("retained request"));
        self.result = Some(result);
        result.map_err(|_| Gfx942SdmaErrorV1::QueueDestroyIndeterminate)?;
        if self.request != Some(original) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "kernel changed immutable SDMA DESTROY_QUEUE inputs",
            ));
        }
        memory
            .sdma_release_doorbell(
                owner.doorbell.as_mut().expect("preflight doorbell"),
                &mut self.doorbell,
            )
            .map_err(doorbell_error)?;
        owner.destroyed = true;
        memory.sdma_release_currentness()?;
        owner.poisoned = false;
        Ok(())
    }

    pub(super) fn release_resources_in_place(
        &mut self,
        owner: &mut Gfx942SdmaQueueOwnerV1,
        memory: &mut impl SdmaOwnerReleaseMemoryV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        // Extraction is infallible after preflight; root all three resources
        // before the first effectful unmap/release operation.
        self.resources = Some(SdmaResourceCleanupCustodyV1::new_sdma(
            owner.completions.take().expect("preflight completions"),
            owner
                .control
                .take()
                .expect("preflight control")
                .into_token(),
            owner.ring.take().expect("preflight ring").into_token(),
        ));
        let resources = self.resources.as_mut().expect("rooted resources");
        memory.sdma_release_resources(resources)?;
        if !resources.is_complete() {
            return Err(Gfx942SdmaErrorV1::Contract("incomplete SDMA resources"));
        }
        Ok(())
    }
}

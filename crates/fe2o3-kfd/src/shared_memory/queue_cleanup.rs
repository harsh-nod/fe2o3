//! Fixed-order, retained cleanup of the four ordinary primary-queue resources.

use super::*;

pub(crate) struct QueueResourceCleanupCustodyV1 {
    controls: [ControlCleanupCustodyV1; 4],
    started: bool,
    failed: bool,
    unmapped: usize,
    released: usize,
}

impl QueueResourceCleanupCustodyV1 {
    pub(crate) fn new(
        ring: SharedGttAllocationV1<AqlQueueGttV1, GttGpuAccessibleMutableV1>,
        control: SharedGttAllocationV1<UserptrAqlControlGttV1, GttGpuAccessibleMutableV1>,
        eop: SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
        context_save: SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
    ) -> Self {
        Self {
            controls: [
                ControlCleanupCustodyV1::aql_ring(ring),
                ControlCleanupCustodyV1::aql_control(control),
                ControlCleanupCustodyV1::code(eop),
                ControlCleanupCustodyV1::code(context_save),
            ],
            started: false,
            failed: false,
            unmapped: 0,
            released: 0,
        }
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.started
            && !self.failed
            && self.unmapped == self.controls.len()
            && self.released == self.controls.len()
            && self
                .controls
                .iter()
                .all(ControlCleanupCustodyV1::is_complete)
    }
}

pub(super) fn release_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut control_cleanup::ProjectionV1<'_>,
    custody: &mut QueueResourceCleanupCustodyV1,
    mut process_poison: impl FnMut(),
) -> Result<(), MemorySessionError> {
    if custody.started {
        return Err(MemorySessionError::InvalidAllocationAuthority);
    }
    custody.started = true;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for control in &mut custody.controls {
            control_cleanup::unmap_v1(engine, projection, control, &mut process_poison)?;
            custody.unmapped += 1;
        }
        for control in &mut custody.controls {
            control_cleanup::finish_release_v1(engine, projection, control, &mut process_poison)?;
            custody.released += 1;
        }
        Ok(())
    }));
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            custody.failed = true;
            engine.quarantine(error)
        }
        Err(payload) => {
            custody.failed = true;
            engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            std::panic::resume_unwind(payload)
        }
    }
}

#[cfg(test)]
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct QueueResourceCleanupObservationV1 {
    pub(crate) controls: [control_cleanup::ControlCleanupObservationV1; 4],
    pub(crate) started: bool,
    pub(crate) failed: bool,
    pub(crate) unmapped: usize,
    pub(crate) released: usize,
}

#[cfg(test)]
impl QueueResourceCleanupCustodyV1 {
    pub(crate) fn observation(&self) -> QueueResourceCleanupObservationV1 {
        QueueResourceCleanupObservationV1 {
            controls: std::array::from_fn(|i| self.controls[i].observation()),
            started: self.started,
            failed: self.failed,
            unmapped: self.unmapped,
            released: self.released,
        }
    }
}

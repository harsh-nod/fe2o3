//! Fixed-order, retained queue cleanup: all unmaps precede all disposals.

use super::*;

pub(crate) struct QueueResourceCleanupCustodyV1<const N: usize = 4> {
    controls: [ControlCleanupCustodyV1; N],
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
}

pub(crate) type SdmaResourceCleanupCustodyV1 = QueueResourceCleanupCustodyV1<3>;

impl SdmaResourceCleanupCustodyV1 {
    pub(crate) fn new_sdma(
        completions: SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>,
        control: SharedGttAllocationV1<UserptrAqlControlGttV1, GttGpuAccessibleMutableV1>,
        ring: SharedGttAllocationV1<AqlQueueGttV1, GttGpuAccessibleMutableV1>,
    ) -> Self {
        Self {
            controls: [
                ControlCleanupCustodyV1::host_data(completions),
                ControlCleanupCustodyV1::aql_control(control),
                ControlCleanupCustodyV1::aql_ring(ring),
            ],
            started: false,
            failed: false,
            unmapped: 0,
            released: 0,
        }
    }
}

impl<const N: usize> QueueResourceCleanupCustodyV1<N> {
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

pub(super) fn release_v1<B: MemoryBackend, const N: usize>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut control_cleanup::ProjectionV1<'_>,
    custody: &mut QueueResourceCleanupCustodyV1<N>,
    mut process_poison: impl FnMut(),
) -> Result<(), MemorySessionError> {
    if custody.started {
        return Err(MemorySessionError::InvalidAllocationAuthority);
    }
    custody.started = true;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SDMA admits the whole six-transition batch before its first unmap.
        // The four-resource primary profile retains its existing per-step policy.
        if N == 3 {
            projection.preflight_revisions(engine, 6, &mut process_poison)?;
        }
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
pub(crate) struct QueueResourceCleanupObservationV1<const N: usize = 4> {
    pub(crate) controls: [control_cleanup::ControlCleanupObservationV1; N],
    pub(crate) started: bool,
    pub(crate) failed: bool,
    pub(crate) unmapped: usize,
    pub(crate) released: usize,
}

#[cfg(test)]
impl<const N: usize> QueueResourceCleanupCustodyV1<N> {
    pub(crate) fn observation(&self) -> QueueResourceCleanupObservationV1<N> {
        QueueResourceCleanupObservationV1 {
            controls: std::array::from_fn(|i| self.controls[i].observation()),
            started: self.started,
            failed: self.failed,
            unmapped: self.unmapped,
            released: self.released,
        }
    }
}

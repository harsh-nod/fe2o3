//! Concrete I/O providers; receipt settlement remains in the indexed callers.

use super::*;

pub(super) enum OrdinaryQueueIoV1<'a> {
    Native(&'a mut ComputeAqlQueueSessionV1),
    #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
    Cpu(&'a mut CpuOrdinaryQueueV1),
}

pub(super) enum OrdinaryLaneIoV1<'a, 'b> {
    Native(&'a mut ComputeAqlQueueLaneDispatchV1<'b>),
    #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
    Cpu(
        &'a mut fe2o3_kfd::CpuFixedDispatchLaneV1<'b>,
        &'a mut CpuLaneControlV1,
    ),
}

impl<'a> OrdinaryQueueIoV1<'a> {
    pub(super) fn new(
        native: Option<&'a mut ComputeAqlQueueSessionV1>,
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))] cpu: Option<&'a mut CpuOrdinaryQueueV1>,
    ) -> Result<Self, ComputeAqlQueueSessionErrorV1> {
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        if let Some(cpu) = cpu {
            return if native.is_none() {
                Ok(Self::Cpu(cpu))
            } else {
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "conflicting ordinary queue providers",
                ))
            };
        }
        native
            .map(Self::Native)
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing ordinary queue provider",
            ))
    }

    pub(super) fn with_lane<R>(
        self,
        lane: ComputeAqlQueueLaneV1,
        operation: impl FnOnce(&mut OrdinaryLaneIoV1<'_, '_>) -> R,
    ) -> Result<R, ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Native(queue) => queue.with_compute_lane_v1(lane, |selected| {
                operation(&mut OrdinaryLaneIoV1::Native(selected))
            }),
            #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
            Self::Cpu(queue) => {
                let next_fault = &mut queue.next_outer_fault;
                let control = &mut queue.lane_control;
                let deposited = &mut queue.before_outer_fault;
                let mut fault = None;
                let result = queue.fixture.with_lane(lane, |selected| {
                    control.returned_operation = None;
                    let result = operation(&mut OrdinaryLaneIoV1::Cpu(selected, control));
                    if next_fault
                        .as_ref()
                        .is_some_and(|(target, _)| Some(*target) == control.returned_operation)
                    {
                        fault = next_fault.take().map(|(_, fault)| fault);
                    }
                    if fault.is_some() {
                        *deposited = Some(selected.snapshot());
                    }
                    if fault == Some(CpuOuterFaultV1::Unwind) {
                        std::panic::panic_any(CpuOuterFaultV1::Unwind);
                    }
                    result
                })?;
                if fault == Some(CpuOuterFaultV1::Error) {
                    return Err(ComputeAqlQueueSessionErrorV1::Contract(
                        "CPU outer lane close fault",
                    ));
                }
                Ok(result)
            }
        }
    }
}

impl OrdinaryLaneIoV1<'_, '_> {
    pub(super) fn submit_classified(
        &mut self,
    ) -> Result<Gfx942DispatchBatchV1<1>, Gfx942FixedDispatchSubmissionFailureV1> {
        match self {
            Self::Native(lane) => lane.submit_fixed_dispatch_classified_v1::<1>(),
            #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
            Self::Cpu(lane, control) => {
                let result = if core::mem::take(&mut control.pin_next_submission) {
                    // Pin setup has no classified retry witness; callers fail closed on error.
                    lane.submit_pinned()
                        .map_err(Gfx942FixedDispatchSubmissionFailureV1::Terminal)
                } else {
                    lane.submit()
                };
                control.returned_operation = Some(CpuIoOperationV1::Submit);
                if let Ok(batch) = &result {
                    // Observe without introducing an error/panic boundary before deposit.
                    control.last_submitted_identity = Some(lane.identity(batch));
                }
                result
            }
        }
    }

    pub(super) fn poll(
        &mut self,
        batch: Gfx942DispatchBatchV1<1>,
    ) -> Result<Gfx942DispatchPollV1<1>, ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Native(lane) => lane.poll_fixed_dispatch(batch),
            #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
            Self::Cpu(lane, control) => {
                let result = lane.poll(batch);
                control.returned_operation = Some(CpuIoOperationV1::Poll);
                result
            }
        }
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn recycle(
        &mut self,
        completed: Gfx942CompletedDispatchBatchV1<1>,
    ) -> Result<Gfx942CompletionRecycleObservationV1, Gfx942FixedDispatchRecycleFailureV1<1>> {
        match self {
            Self::Native(lane) => lane.recycle_fixed_dispatch(completed),
            #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
            Self::Cpu(lane, control) => {
                let result = lane.recycle(completed);
                control.returned_operation = Some(CpuIoOperationV1::Recycle);
                if result.as_ref().is_err_and(|failure| {
                    matches!(
                        failure.error(),
                        ComputeAqlQueueSessionErrorV1::Completion(
                            fe2o3_kfd::Gfx942CompletionErrorV1::SignalPinned {
                                event_pins: 1,
                                native_reader_pins: 0,
                                ..
                            }
                        )
                    )
                }) {
                    control.pinned_recycles = control.pinned_recycles.saturating_add(1);
                }
                result
            }
        }
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn ordinary_queue_available_v1(&self) -> bool {
        let native = self.queue.is_some();
        #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
        return native != self.cpu_queue.is_some();
        #[cfg(not(all(test, feature = "cpu-runtime-fixtures")))]
        native
    }
}

#[cfg(all(test, feature = "cpu-runtime-fixtures"))]
pub(super) struct CpuOrdinaryQueueV1 {
    pub(super) fixture: fe2o3_kfd::CpuFixedDispatchFixtureV1,
    pub(super) next_outer_fault: Option<(CpuIoOperationV1, CpuOuterFaultV1)>,
    pub(super) lane_control: CpuLaneControlV1,
    pub(super) before_outer_fault: Option<fe2o3_kfd::CpuLaneSnapshotV1>,
}

#[cfg(all(test, feature = "cpu-runtime-fixtures"))]
#[derive(Default)]
pub(super) struct CpuLaneControlV1 {
    pub(super) pin_next_submission: bool,
    pub(super) pinned_recycles: usize,
    returned_operation: Option<CpuIoOperationV1>,
    pub(super) last_submitted_identity:
        Option<Result<fe2o3_kfd::CpuDispatchIdentityV1, ComputeAqlQueueSessionErrorV1>>,
}

#[cfg(all(test, feature = "cpu-runtime-fixtures"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CpuIoOperationV1 {
    Submit,
    Poll,
    Recycle,
}

#[cfg(all(test, feature = "cpu-runtime-fixtures"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CpuOuterFaultV1 {
    Error,
    Unwind,
}

#[cfg(all(test, feature = "cpu-runtime-fixtures"))]
impl KfdRuntimeBackendV1 {
    pub(super) fn require_cpu_provider_v1(
        &self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(queue) = self.cpu_queue.as_ref() else {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "CPU receipt fixture is absent",
            ));
        };
        let handles = [queue.fixture.primary_lane(), queue.fixture.auxiliary_lane()];
        if self.queue.is_some()
            || self.admitted_device.is_some()
            || self.primary_teardown.is_some()
            || self.terminal_memory.is_some()
            || self.retained_persistent_dispatch.is_some()
            || self.scripted_sdma.is_some()
            || self.scripted_materialized_preparation.is_some()
            || self.scripted_materialized_publication_fault.is_some()
            || self.scripted_materialized_completion.is_some()
            || self.scripted_ordered_publication.is_some()
            || self.sdma_enabled
            || self.native_compute_lanes.len() < handles.len()
            || self
                .native_compute_lanes
                .iter()
                .enumerate()
                .any(|(index, lane)| *lane != handles.get(index).copied())
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "CPU receipt fixture conflicts with native or scripted custody",
            ));
        }
        Ok(())
    }

    pub(super) fn detach_cpu_recycled_dispatch_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_cpu_provider_v1()?;
        self.require_unpinned_native_lane_v1(self.selected_compute_lane)?;
        if self.active.is_some()
            || !self.compute_pipeline.is_empty()
            || self.resident_data.is_some()
            || self.recycled_dispatch.as_ref().is_some_and(|cache| {
                cache
                    .descriptors
                    .iter()
                    .any(|descriptor| descriptor.device_may_have_modified)
            })
            || self.native_dirty_extents != 0
            || self
                .allocations
                .ordinary_iter()
                .any(|(_, allocation)| !allocation.native_dirty.is_empty())
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "CPU receipt cache still has live or device DATA custody",
            ));
        }
        if self.recycled_dispatch.is_none() {
            return Ok(());
        }
        // No native DATA is detached here. Retiring a cache requires both lanes idle.
        self.cpu_queue
            .as_ref()
            .unwrap()
            .fixture
            .ensure_clean()
            .map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "CPU lower receipt custody remains live",
                )
            })?;
        self.recycled_dispatch = None;
        Ok(())
    }

    pub(super) fn shutdown_cpu_queue_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_cpu_provider_v1()?;
        if self.resident_data.is_some()
            || self.recycled_dispatch.is_some()
            || self
                .auxiliary_compute_lanes
                .iter()
                .any(|lane| lane.resident_data.is_some() || lane.recycled_dispatch.is_some())
            || self.native_dirty_extents != 0
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "CPU logical caches must be released before shutdown",
            ));
        }
        self.cpu_queue
            .as_ref()
            .unwrap()
            .fixture
            .ensure_clean()
            .map_err(|_| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "CPU lower receipt custody remains live",
                )
            })?;
        self.cpu_queue = None;
        self.native_compute_lanes.fill(None);
        self.native_available = false;
        self.queue_retired = true;
        Ok(())
    }
}

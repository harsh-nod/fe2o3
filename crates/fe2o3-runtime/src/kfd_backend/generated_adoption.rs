//! Backend-rooted, nonpublishing generated DATA adoption and retirement.

use super::*;
use crate::generated_source::GeneratedHostRosterV1;
use generated_shells::GeneratedShellPlanV1;
use std::panic::{AssertUnwindSafe, catch_unwind};

mod issue;
mod receipt;
use receipt::ReceiptV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhaseV1 {
    Entering,
    Adopted,
    Retiring,
    Retired,
}

pub(super) struct GeneratedNativeAdoptionV1 {
    phase: PhaseV1,
    lane: usize,
    native_lane: Option<ComputeAqlQueueLaneV1>,
    data: Vec<Gfx942FixedDispatchDataV1>,
    returned: ReturnedDataV1<Gfx942FixedDispatchDataV1>,
    submission: Option<issue::GeneratedSubmissionV1>,
}

// The lower consuming release owns the current item on failure. This root keeps
// the untouched suffix and exact successful prefix, without an unwind-local Vec.
struct ReturnedDataV1<T> {
    remaining: Option<std::vec::IntoIter<T>>,
    completed: usize,
    handed_to_lower: Option<usize>,
}

impl<T> ReturnedDataV1<T> {
    fn empty() -> Self {
        Self {
            remaining: None,
            completed: 0,
            handed_to_lower: None,
        }
    }

    fn install(&mut self, data: Vec<T>) {
        assert!(self.remaining.is_none());
        self.remaining = Some(data.into_iter());
    }

    fn release<E>(&mut self, mut release: impl FnMut(T) -> Result<(), E>) -> Result<(), E> {
        assert!(
            self.handed_to_lower.is_none(),
            "failed DATA release cannot retry"
        );
        for item in self.remaining.as_mut().expect("rooted returned DATA") {
            self.handed_to_lower = Some(self.completed);
            release(item)?;
            self.completed += 1;
            self.handed_to_lower = None;
        }
        Ok(())
    }
}

impl GeneratedNativeAdoptionV1 {
    pub(super) fn disposed_count(&self) -> Option<usize> {
        (self.is_retired() && self.submission.is_none()).then_some(self.returned.completed)
    }

    pub(super) fn is_retired(&self) -> bool {
        self.phase == PhaseV1::Retired
            && self.data.is_empty()
            && self.returned.handed_to_lower.is_none()
            && self
                .returned
                .remaining
                .as_ref()
                .is_some_and(|data| data.len() == 0)
    }
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn resume_generated_adoption_panic_v1(
        &mut self,
        payload: Box<dyn std::any::Any + Send>,
    ) -> ! {
        super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1())
    }

    pub(crate) fn preflight_generated_lane_v1(
        &self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if !self.generated_lane_ready_v1()? {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "generated compute lane unavailable",
            ));
        }
        Ok(())
    }

    pub(crate) fn generated_lane_ready_v1(
        &self,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        Ok(!self.persistent_compute_is_active_v1() && self.free_compute_lane_v1().is_some())
    }

    pub(crate) fn quarantine_generated_adoption_v1(&mut self) {
        self.poison_terminal_v1();
    }

    pub(super) fn has_live_generated_native_v1(&self) -> bool {
        self.generated_shells.values().any(|record| {
            record
                .native
                .as_ref()
                .is_some_and(|native| !native.is_retired())
        })
    }

    pub(super) fn require_no_generated_stream_v1(
        &self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self
            .generated_shells
            .values()
            .any(|record| record.plan.binding.backend_stream == stream)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "generated stream remains held",
            ));
        }
        Ok(())
    }

    pub(crate) fn generated_empty_prefix_v1(&self, stream: u64) -> bool {
        self.require_no_generated_stream_v1(stream).is_ok()
            && !self.stream_compute_lanes.contains_key(&stream)
    }

    pub(crate) fn adopt_generated_data_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        program: ValidatedKernelEnvelope<'_>,
        buffers: &[crate::Gfx942KfdDispatchBufferV1],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        self.preflight_generated_lane_v1()?;
        if !self.validate_generated_shell_records_v1(plan)
            || !self.generated_shells.get(&plan.key).is_some_and(|record| {
                record.native.is_none()
                    && record.control.is_some()
                    && Arc::ptr_eq(&record.source_identity, &roster.source_identity)
            })
            || buffers.len() != plan.count
            || roster.count != plan.count
            || buffers.iter().enumerate().any(|(index, buffer)| {
                plan.members[index]
                    .is_none_or(|member| member.description.byte_len != buffer.bytes().len() as u64)
            })
            || self
                .stream_compute_lanes
                .contains_key(&plan.binding.backend_stream)
            || self
                .pending_compute_streams
                .contains_key(&plan.binding.backend_stream)
            || self
                .active_sdma_streams
                .contains_key(&plan.binding.backend_stream)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "generated adoption identity or state mismatch",
            ));
        }
        let lane = self
            .free_compute_lane_v1()
            .ok_or_else(|| Self::capacity("generated compute lane capacity"))?;
        let current = self
            .with_retained_preparation_device_v1(plan.binding.backend_device, |device| {
                device.model_admission()
            })?;
        if current != plan.binding.native_device {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated native device mismatch",
            ));
        }
        let mut programs = Vec::new();
        programs
            .try_reserve_exact(1)
            .map_err(|_| Self::capacity("generated program roster capacity"))?;
        programs.push(program);
        let mut data = Vec::new();
        data.try_reserve_exact(buffers.len())
            .map_err(|_| Self::capacity("generated DATA roster capacity"))?;
        self.stream_compute_lanes
            .try_reserve(1)
            .map_err(|_| Self::capacity("generated lane lease capacity"))?;
        self.generated_shells
            .get_mut(&plan.key)
            .expect("validated generated shell")
            .native = Some(GeneratedNativeAdoptionV1 {
            phase: PhaseV1::Entering,
            lane,
            native_lane: self.native_compute_lanes[lane],
            data,
            returned: ReturnedDataV1::empty(),
            submission: None,
        });
        self.lease_compute_lane_v1(plan.binding.backend_stream, lane);
        let result = catch_unwind(AssertUnwindSafe(|| {
            if lane == 0 {
                self.release_retained_persistent_control_v1()?;
            }
            self.release_compute_lane_cache_v1(lane)?;
            self.bind_generated_data_v1(plan.key, programs, buffers)?;
            let current = self
                .with_retained_preparation_device_v1(plan.binding.backend_device, |device| {
                    device.model_admission()
                })?;
            if current != plan.binding.native_device {
                return Err(self.terminal_error("generated closing device mismatch"));
            }
            self.generated_shells
                .get_mut(&plan.key)
                .expect("rooted generated shell")
                .native
                .as_mut()
                .expect("native entry")
                .phase = PhaseV1::Adopted;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }

    fn bind_generated_data_v1(
        &mut self,
        key: u64,
        programs: Vec<ValidatedKernelEnvelope<'_>>,
        buffers: &[crate::Gfx942KfdDispatchBufferV1],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let record = self
            .generated_shells
            .get_mut(&key)
            .expect("rooted generated shell");
        let control = &mut record.control;
        let native = record.native.as_mut().expect("native entry");
        let lane = native.lane;
        let created = native.native_lane.is_none();
        let Some(queue) = self.queue.as_mut() else {
            assert!(self.terminal_memory.is_none());
            let device = self
                .admitted_device
                .take()
                .expect("validated retained device");
            let memory = device
                .acquire_shared_gtt_memory_session_with_backing_budgets_v1(
                    self.device_backing_budget,
                    self.host_visible_backing_budget,
                )
                .map_err(|error| self.generated_native_error_v1("VM acquisition", error))?;
            self.terminal_memory = Some(memory);
            let native = self
                .generated_shells
                .get_mut(&key)
                .expect("rooted generated shell")
                .native
                .as_mut()
                .expect("native entry");
            let memory = self.terminal_memory.as_mut().expect("rooted generated VM");
            initialize_generated_data_v1(memory, buffers, &mut native.data)
                .map_err(|error| self.generated_native_error_v1("DATA initialization", error))?;
            let data = core::mem::take(
                &mut self
                    .generated_shells
                    .get_mut(&key)
                    .expect("rooted shell")
                    .native
                    .as_mut()
                    .expect("native entry")
                    .data,
            );
            let packet = self
                .generated_shells
                .get_mut(&key)
                .expect("rooted shell")
                .control
                .take()
                .expect("original one-shot packet");
            let queue = self
                .terminal_memory
                .take()
                .expect("rooted generated VM")
                .create_compute_aql_queue_with_fixed_dispatch(
                    KFD_RUNTIME_RING_BYTES_V1,
                    programs,
                    [packet],
                    data,
                )
                .map_err(|error| self.generated_native_error_v1("primary construction", error))?;
            let handle = queue.primary_compute_lane_v1();
            self.queue = Some(queue);
            self.native_compute_lanes[lane] = Some(handle);
            self.generated_shells
                .get_mut(&key)
                .expect("rooted shell")
                .native
                .as_mut()
                .expect("native entry")
                .native_lane = Some(handle);
            self.observe_generated_queue_creation_v1(lane);
            self.configure_native_device_pool_v1()?;
            self.configure_native_host_pool_v1()?;
            return Ok(());
        };
        let result = if let Some(handle) = native.native_lane {
            queue
                .with_compute_lane_v1(handle, |queue| {
                    for (index, buffer) in buffers.iter().enumerate() {
                        let data = queue
                            .insert_initialized_host_visible_fixed_dispatch_data_from_slice_v1(
                                index,
                                buffer.bytes(),
                            )?;
                        native.data.push(data);
                    }
                    queue.bind_fixed_dispatch(
                        programs,
                        [control.take().expect("original one-shot packet")],
                        core::mem::take(&mut native.data),
                    )
                })
                .and_then(core::convert::identity)
                .map(|()| handle)
        } else if self.native_compute_lanes.iter().all(Option::is_none) {
            queue
                .bind_initial_fixed_dispatch_v1(
                    programs,
                    [control.take().expect("original one-shot packet")],
                    buffers.len(),
                    |memory, index| {
                        memory
                            .initialize_host_visible_coherent_from_slice_v1(buffers[index].bytes())
                            .map(Gfx942FixedDispatchDataV1::host_visible_initialized)
                            .map_err(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Memory)
                    },
                )
                .map(|()| queue.primary_compute_lane_v1())
        } else {
            queue.create_auxiliary_compute_lane_with_fixed_dispatch(
                KFD_RUNTIME_RING_BYTES_V1,
                programs,
                [control.take().expect("original one-shot packet")],
                |memory| {
                    initialize_generated_data_v1(memory, buffers, &mut native.data)?;
                    Ok(core::mem::take(&mut native.data))
                },
            )
        };
        let handle =
            result.map_err(|error| self.generated_native_error_v1("native binding", error))?;
        self.native_compute_lanes[lane] = Some(handle);
        self.generated_shells
            .get_mut(&key)
            .expect("rooted shell")
            .native
            .as_mut()
            .expect("native entry")
            .native_lane = Some(handle);
        if created {
            self.observe_generated_queue_creation_v1(lane);
        }
        Ok(())
    }

    fn observe_generated_queue_creation_v1(&mut self, lane: usize) {
        let queue = self.profile_resource_v1(
            KfdProfileResourceKindV1::NativeQueue,
            KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + lane as u64,
        );
        self.observe_profile_v1(
            queue.map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue }),
        );
    }

    fn generated_lease_matches_v1(&self, plan: &GeneratedShellPlanV1) -> bool {
        self.generated_shells
            .get(&plan.key)
            .and_then(|record| record.native.as_ref())
            .is_some_and(|native| {
                self.streams.get(&plan.binding.backend_stream) == Some(&plan.binding.backend_device)
                    && self.stream_compute_lanes.get(&plan.binding.backend_stream)
                        == Some(&native.lane)
                    && native.native_lane.is_some()
                    && self.native_compute_lanes.get(native.lane) == Some(&native.native_lane)
                    && (native.lane == 0
                        || self
                            .auxiliary_compute_lanes
                            .get(native.lane - 1)
                            .is_some_and(|lane| {
                                lane.owner_stream == Some(plan.binding.backend_stream)
                            }))
            })
    }

    pub(crate) fn retire_generated_data_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.validate_generated_shell_records_v1(plan) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated retirement roster mismatch",
            ));
        }
        let record = self
            .generated_shells
            .get(&plan.key)
            .expect("validated shell");
        let Some(native) = record.native.as_ref() else {
            return if record.control.is_some() {
                Ok(())
            } else {
                Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "missing generated control",
                ))
            };
        };
        let recycled = native
            .submission
            .as_ref()
            .is_some_and(|submission| matches!(submission.receipt, ReceiptV1::Recycled));
        let unpublished = native
            .submission
            .as_ref()
            .is_none_or(|submission| matches!(submission.receipt, ReceiptV1::Ready));
        if native.phase != PhaseV1::Adopted
            || (!recycled && !unpublished)
            || !self.generated_lease_matches_v1(plan)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "generated retirement phase or lease mismatch",
            ));
        }
        let lane_index = native.lane;
        let handle = native.native_lane.expect("exact generated lane");
        self.generated_shells
            .get_mut(&plan.key)
            .expect("rooted shell")
            .native
            .as_mut()
            .expect("native entry")
            .phase = PhaseV1::Retiring;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .expect("rooted shell")
                .native
                .as_mut()
                .expect("native entry");
            let result = self
                .queue
                .as_mut()
                .expect("retained generated queue")
                .with_compute_lane_v1(handle, |lane| {
                    let data = if recycled {
                        lane.detach_recycled_fixed_dispatch()?.into_data()
                    } else {
                        lane.abort_unpublished_fixed_dispatch_v1()?
                    };
                    native.returned.install(data);
                    if native
                        .returned
                        .remaining
                        .as_ref()
                        .expect("rooted DATA")
                        .len()
                        != plan.count
                    {
                        return Err(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                            "generated returned DATA count mismatch",
                        ));
                    }
                    native
                        .returned
                        .release(|data| lane.release_detached_fixed_dispatch_data(data))
                })
                .and_then(core::convert::identity);
            result.map_err(|error| self.generated_native_error_v1("native retirement", error))?;
            let current = self
                .with_retained_preparation_device_v1(plan.binding.backend_device, |device| {
                    device.model_admission()
                })?;
            if current != plan.binding.native_device || !self.generated_lease_matches_v1(plan) {
                return Err(self.terminal_error("generated closing retirement identity mismatch"));
            }
            let native = self
                .generated_shells
                .get_mut(&plan.key)
                .expect("rooted shell")
                .native
                .as_mut()
                .expect("native entry");
            if native.returned.completed != plan.count || !native.data.is_empty() {
                return Err(self.terminal_error("generated retirement DATA count mismatch"));
            }
            native.phase = PhaseV1::Retired;
            self.release_compute_lane_lease_v1(plan.binding.backend_stream, lane_index);
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }

    fn finish_generated_native_call_v1(
        &mut self,
        result: std::thread::Result<Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>>,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(
                RuntimeBackendFailureV1::Rejected(error)
                | RuntimeBackendFailureV1::Quiescent(error)
                | RuntimeBackendFailureV1::Terminal(error),
            )) => {
                self.poison_terminal_v1();
                Err(RuntimeBackendFailureV1::Terminal(error))
            }
            Err(payload) => self.resume_generated_adoption_panic_v1(payload),
        }
    }

    fn generated_native_error_v1(
        &mut self,
        stage: &str,
        error: impl fmt::Display,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        self.poison_terminal_v1();
        self.terminal_error(format!("generated {stage}: {error}"))
    }
}

fn initialize_generated_data_v1(
    memory: &mut SharedGttMemorySessionV1,
    buffers: &[crate::Gfx942KfdDispatchBufferV1],
    data: &mut Vec<Gfx942FixedDispatchDataV1>,
) -> Result<(), fe2o3_kfd::ComputeAqlQueueSessionErrorV1> {
    for buffer in buffers {
        let item = memory
            .initialize_host_visible_coherent_from_slice_v1(buffer.bytes())
            .map_err(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Memory)?;
        data.push(Gfx942FixedDispatchDataV1::host_visible_initialized(item));
    }
    Ok(())
}

#[cfg(test)]
mod tests;

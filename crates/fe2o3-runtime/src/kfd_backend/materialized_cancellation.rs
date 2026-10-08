//! Retire an ordinary retry without attributing a prior completion to this launch.

use super::*;

pub(super) fn materialized_descriptor_projection_intact_v1(
    bindings: &[BackendBindingV1],
    descriptors: &[ResidentDataDescriptorV1],
    allocations: &AllocationTableV1,
) -> bool {
    if descriptors.len() > GFX942_MAX_FIXED_DISPATCH_DATA_V1
        || bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1
    {
        return false;
    }
    // Merge aliases in descriptor slots, preserving first-binding order without
    // rescanning binding prefixes. Native DATA bounds this work to B * 16.
    let mut ranges = [None::<(u64, u64)>; GFX942_MAX_FIXED_DISPATCH_DATA_V1];
    let mut count = 0;
    for binding in bindings {
        let allocation = binding.region.allocation;
        let Some(index) = descriptors.iter().position(|d| d.allocation == allocation) else {
            return false;
        };
        let Some(record) = allocations.get(&allocation) else {
            return false;
        };
        if !record.alignment.is_power_of_two() {
            return false;
        }
        let Some(end) = binding
            .region
            .byte_offset
            .checked_add(binding.region.byte_len)
        else {
            return false;
        };
        if binding.region.byte_len == 0 || end > record.bytes.len() as u64 {
            return false;
        }
        let start = binding.region.byte_offset & !(record.alignment - 1);
        ranges[index] = Some(match ranges[index] {
            Some((prior_start, prior_end)) => (start.min(prior_start), end.max(prior_end)),
            None => {
                if index != count {
                    return false;
                }
                count += 1;
                (start, end)
            }
        });
    }
    if count != descriptors.len() {
        return false;
    }
    descriptors.iter().enumerate().all(|(index, descriptor)| {
        let Some((start, end)) = ranges[index] else {
            return false;
        };
        let record = &allocations[&descriptor.allocation];
        descriptor.kind == record.kind
            && descriptor.alignment == record.alignment
            && descriptor.allocation_offset == start
            && descriptor.byte_len == end - start
            && !descriptor.device_may_have_modified
    })
}

#[cfg(test)]
#[path = "tests/materialized_descriptor_tests.rs"]
mod descriptor_tests;

pub(super) fn materialized_return_layout_matches_v1(
    descriptor: &ResidentDataDescriptorV1,
    kind: fe2o3_kfd::Gfx942FixedDispatchDataKindV1,
    byte_len: u64,
    alignment: u64,
) -> bool {
    let (expected_kind, expected_alignment) = match descriptor.kind {
        RuntimeMemoryKindV1::HostVisible => (
            fe2o3_kfd::Gfx942FixedDispatchDataKindV1::HostVisibleCoherent,
            HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
        ),
        RuntimeMemoryKindV1::DeviceLocal => (
            fe2o3_kfd::Gfx942FixedDispatchDataKindV1::DeviceLocal,
            descriptor.alignment,
        ),
    };
    kind == expected_kind && byte_len == descriptor.byte_len && alignment == expected_alignment
}

pub(super) struct MaterializedCancellationV1 {
    pub(super) prepared: MaterializedPreparedV1,
    pub(super) returned: Option<Vec<Gfx942FixedDispatchDataV1>>,
    pub(super) retired_generation: Option<u64>,
    #[cfg(test)]
    pub(super) scripted_returned: bool,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScriptedMaterializedCancelFaultV1 {
    BeforeReturn,
    AfterReturn,
    WrongGeneration,
}

impl KfdRuntimeBackendV1 {
    fn materialized_cancel_root_v1(&self) -> &MaterializedCancellationV1 {
        match self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        {
            Some(ActiveComputeExecutionV1::MaterializedCancelling(root)) => root,
            _ => unreachable!("indexed materialized cancellation root"),
        }
    }

    pub(super) fn materialized_prepared_custody_intact_v1(&self, submission: u64) -> bool {
        let Some(active) = self.active.as_ref() else {
            return false;
        };
        let Some(recipe) = active.ordinary_recipe.as_ref() else {
            return false;
        };
        let Some(kernel) = self.kernels.get(&active.kernel) else {
            return false;
        };
        let lane = self.selected_compute_lane;
        if submission == 0
            || !active.source_event.may_retry()
            || active.id != submission
            || recipe.stream != active.stream
            || recipe.kernel != active.kernel
            || recipe.bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1
            || active.deferred_ordered_predecessor_retain
            || !self.compute_pipeline.is_empty()
            || self.resident_data.is_some()
            || self.recycled_dispatch.is_some()
            || self.stream_compute_lanes.get(&active.stream) != Some(&lane)
            || (lane != 0
                && self
                    .auxiliary_compute_lanes
                    .get(lane - 1)
                    .is_none_or(|aux| aux.owner_stream != Some(active.stream)))
            || self
                .stream_submission_tails
                .get(&active.stream)
                .is_none_or(|tail| *tail < submission)
            || self.compute_completion_reservations == 0
            || self.submissions.contains_key(&submission)
            || self.pending_compute.contains_key(&submission)
            || self.active_sdma.contains_key(&submission)
            || self
                .compute_module_retain_counts
                .get(&kernel.module)
                .is_none_or(|count| *count == 0)
            || self
                .modules
                .get(&kernel.module)
                .is_none_or(|module| self.streams.get(&active.stream) != Some(&module.device))
        {
            return false;
        }
        let prepared = match active.execution.as_ref() {
            Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) => prepared,
            Some(ActiveComputeExecutionV1::MaterializedCancelling(root)) => &root.prepared,
            _ => return false,
        };
        if matches!(
            prepared.origin,
            MaterializedPreparationOriginV1::RecycledAttachment { generation: 0 }
        ) {
            return false;
        }
        if active.allocations.len() != active.resident_descriptors.len()
            || !materialized_descriptor_projection_intact_v1(
                &recipe.bindings,
                &active.resident_descriptors,
                &self.allocations,
            )
        {
            return false;
        }
        for descriptor in &active.resident_descriptors {
            let allocation = descriptor.allocation;
            let expected = RuntimeAllocationCustodyOwnerV1 {
                submission,
                stream: active.stream,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            };
            if !active.allocations.contains(&allocation)
                || !self
                    .allocation_custody
                    .get(&allocation)
                    .is_some_and(|custody| {
                        custody.owner_counts[RuntimeAllocationCustodyKindV1::Compute.index()] != 0
                            && custody
                                .owners
                                .binary_search_by_key(&submission, |owner| owner.submission)
                                .is_ok_and(|index| {
                                    custody.owners[index] == expected
                                        && index
                                            .checked_sub(1)
                                            .and_then(|prior| custody.owners.get(prior))
                                            .is_none_or(|owner| owner.submission != submission)
                                        && custody
                                            .owners
                                            .get(index + 1)
                                            .is_none_or(|owner| owner.submission != submission)
                                })
                    })
            {
                return false;
            }
        }
        true
    }

    pub(super) fn cancel_materialized_prepared_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        // Never retire a recipe while any successor still occupies its pipeline.
        if !self.compute_pipeline.is_empty() {
            return Ok(crate::BackendCancellationV1::TooLate);
        }
        if !self.materialized_prepared_custody_intact_v1(submission) {
            return Err(self.terminal_error("materialized cancellation lost logical custody"));
        }
        let native = true;
        #[cfg(test)]
        let native = native
            && matches!(
                self.active.as_ref().unwrap().execution.as_ref(),
                Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) if prepared.scripted.is_none()
            );
        let native_lane =
            if native {
                if self.queue.is_none() {
                    return Err(self.terminal_error("materialized cancellation lost native queue"));
                }
                Some(self.selected_native_compute_lane_v1().map_err(|_| {
                    self.terminal_error("materialized cancellation lost native lane")
                })?)
            } else {
                None
            };
        self.submissions
            .try_reserve(self.compute_completion_reservations)
            .map_err(|_| Self::capacity("materialized cancellation cannot reserve result"))?;
        let shell = try_uninit_box_v1()
            .map_err(|_| Self::capacity("materialized cancellation cannot reserve root"))?;
        let active = self.active.as_mut().unwrap();
        let Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) =
            active.execution.take()
        else {
            unreachable!("preflighted prepared owner")
        };
        active.execution = Some(ActiveComputeExecutionV1::MaterializedCancelling(
            Box::write(
                shell,
                MaterializedCancellationV1 {
                    prepared,
                    returned: None,
                    retired_generation: None,
                    #[cfg(test)]
                    scripted_returned: false,
                },
            ),
        ));
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if let Some(native_lane) = native_lane {
                    let Some(ActiveComputeExecutionV1::MaterializedCancelling(root)) =
                        self.active.as_mut().unwrap().execution.as_mut()
                    else {
                        unreachable!()
                    };
                    self.queue.as_mut().unwrap().with_compute_lane_v1(native_lane, |lane| {
                    match root.prepared.origin {
                        MaterializedPreparationOriginV1::NewBinding => {
                            root.returned = Some(lane.abort_cancelled_fixed_dispatch_v1()?);
                        }
                        MaterializedPreparationOriginV1::RecycledAttachment { generation } => {
                            if lane.recycled_fixed_dispatch_generation()? != generation {
                                return Err(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                                    "materialized cancellation recycled generation changed",
                                ));
                            }
                            let detached = lane.detach_recycled_fixed_dispatch()?;
                            root.retired_generation = Some(detached.dispatch_generation());
                            root.returned = Some(detached.into_data());
                        }
                    }
                    // Returned authorities are indexed before the outer lane loan closes.
                    Ok(())
                }).map_err(|error| format!("materialized cancellation lane selection: {error}"))?
                    .map_err(|error| format!("materialized cancellation retirement: {error}"))?;
                }
                #[cfg(test)]
                if !native {
                    self.retire_scripted_materialized_v1()?;
                }
                Ok::<(), String>(())
            }));
        match result {
            Ok(Ok(())) => self.commit_materialized_cancellation_v1(submission),
            Ok(Err(detail)) => Err(self.terminal_error(detail)),
            Err(payload) => {
                let _ =
                    self.terminal_error("materialized cancellation unwound with indexed custody");
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn materialized_cancel_return_intact_v1(&self) -> bool {
        let root = self.materialized_cancel_root_v1();
        let generation_matches = match root.prepared.origin {
            MaterializedPreparationOriginV1::NewBinding => root.retired_generation.is_none(),
            MaterializedPreparationOriginV1::RecycledAttachment { generation } => {
                root.retired_generation == Some(generation)
            }
        };
        if !generation_matches {
            return false;
        }
        let descriptors = &self.active.as_ref().unwrap().resident_descriptors;
        #[cfg(test)]
        if let Some((data, _)) = &root.prepared.scripted {
            return root.scripted_returned
                && root.returned.is_none()
                && data.len() == descriptors.len()
                && data.iter().zip(descriptors).all(|(data, descriptor)| {
                    data.allocation == descriptor.allocation
                        && data.kind == descriptor.kind
                        && data.bytes().len() as u64 == descriptor.byte_len
                        && data.content_sha256 == descriptor.host_content_sha256
                });
        }
        root.returned.as_ref().is_some_and(|data| {
            data.len() == descriptors.len()
                && data.iter().zip(descriptors).all(|(data, descriptor)| {
                    let layout = data.layout();
                    data.is_fully_initialized()
                        && materialized_return_layout_matches_v1(
                            descriptor,
                            layout.kind(),
                            layout.requested_bytes(),
                            layout.alignment(),
                        )
                })
        })
    }

    fn commit_materialized_cancellation_v1(
        &mut self,
        submission: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        if !self.materialized_prepared_custody_intact_v1(submission)
            || !self.materialized_cancel_return_intact_v1()
            || self
                .submissions
                .capacity()
                .saturating_sub(self.submissions.len())
                < self.compute_completion_reservations
        {
            return Err(self.terminal_error("materialized cancellation lost retirement custody"));
        }
        let active = self.active.as_mut().unwrap();
        let (stream, module, depth) = (
            active.stream,
            self.kernels[&active.kernel].module,
            active.dependency_depth,
        );
        let Some(ActiveComputeExecutionV1::MaterializedCancelling(root)) =
            active.execution.as_mut()
        else {
            unreachable!()
        };
        if let Some(data) = root.returned.take() {
            self.resident_data = Some(ResidentDataRosterV1 {
                descriptors: core::mem::take(&mut active.resident_descriptors),
                data,
            });
        }
        // Admission retained each unique binding once; successors keep their own retains.
        let count = active.ordinary_recipe.as_ref().unwrap().bindings.len();
        for index in 0..count {
            let bindings = &self
                .active
                .as_ref()
                .unwrap()
                .ordinary_recipe
                .as_ref()
                .unwrap()
                .bindings;
            let allocation = bindings[index].region.allocation;
            if !bindings[..index]
                .iter()
                .any(|prior| prior.region.allocation == allocation)
            {
                self.release_allocation_custody_v1(allocation, submission);
            }
        }
        self.release_compute_module_retain_v1(module);
        self.submissions.insert(
            submission,
            SubmissionRecordV1 {
                origin: SubmissionOriginV1::Ordinary,
                stream,
                status: BackendPollV1::Failed { code: -2 },
                dependency_depth: depth,
                profile_dispatch_published: false,
            },
        );
        self.compute_completion_reservations -= 1;
        self.release_compute_lane_lease_v1(stream, self.selected_compute_lane);
        self.restore_unfinished_stream_tail_v1(stream, submission);
        self.active = None;
        Ok(crate::BackendCancellationV1::Cancelled)
    }

    #[cfg(test)]
    fn retire_scripted_materialized_v1(&mut self) -> Result<(), String> {
        let fault = self.scripted_materialized_cancel_fault.take();
        if fault == Some(ScriptedMaterializedCancelFaultV1::BeforeReturn) {
            return Err("scripted materialized retirement failure".into());
        }
        let Some(ActiveComputeExecutionV1::MaterializedCancelling(root)) =
            self.active.as_mut().unwrap().execution.as_mut()
        else {
            unreachable!()
        };
        root.scripted_returned = true;
        root.retired_generation = match root.prepared.origin {
            MaterializedPreparationOriginV1::NewBinding => None,
            MaterializedPreparationOriginV1::RecycledAttachment { generation } => Some(generation),
        };
        if fault == Some(ScriptedMaterializedCancelFaultV1::WrongGeneration) {
            root.retired_generation = Some(0);
        }
        if fault == Some(ScriptedMaterializedCancelFaultV1::AfterReturn) {
            panic!("scripted materialized retirement unwind after return");
        }
        Ok(())
    }
}

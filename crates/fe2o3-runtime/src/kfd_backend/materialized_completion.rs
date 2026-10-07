//! Ordinary native observation and retirement keep their logical owner indexed.

use super::*;

pub(super) use super::materialized_completion_receipt::MaterializedConsumeV1;

#[derive(Clone, Copy)]
pub(super) enum MaterializedCompletionTargetV1 {
    Frontier(u64),
    Pipeline(RuntimeComputePipelineIdentityV1),
}

impl MaterializedCompletionTargetV1 {
    fn owner(self, backend: &KfdRuntimeBackendV1) -> Option<&ActiveSubmissionV1> {
        match self {
            Self::Frontier(id) => backend.active.as_ref().filter(|active| active.id == id),
            Self::Pipeline(identity) => backend
                .compute_pipeline
                .entry_v1(identity)
                .map(|e| &e.active),
        }
    }

    fn parts_mut<'a>(
        self,
        frontier: &'a mut Option<ActiveSubmissionV1>,
        pipeline: &'a mut RuntimeComputePipelineV1,
    ) -> (
        &'a mut ActiveSubmissionV1,
        Option<&'a mut RuntimeComputePipelinePhaseV1>,
    ) {
        match self {
            Self::Frontier(id) => (
                frontier
                    .as_mut()
                    .filter(|a| a.id == id)
                    .expect("preflighted frontier"),
                None,
            ),
            Self::Pipeline(identity) => {
                let entry = pipeline
                    .entry_mut_v1(identity)
                    .expect("preflighted pipeline identity");
                (&mut entry.active, Some(&mut entry.phase))
            }
        }
    }
}

fn execution_phase(
    execution: Option<&ActiveComputeExecutionV1>,
) -> Option<RuntimeComputePipelinePhaseV1> {
    match execution? {
        ActiveComputeExecutionV1::Materialized(MaterializedCompletionReceiptV1::Published(_)) => {
            Some(RuntimeComputePipelinePhaseV1::Published)
        }
        ActiveComputeExecutionV1::Materialized(MaterializedCompletionReceiptV1::Completed(_)) => {
            Some(RuntimeComputePipelinePhaseV1::Completed)
        }
        ActiveComputeExecutionV1::Materialized(MaterializedCompletionReceiptV1::Retired(
            observation,
        )) if observation.packet_count() == 1 => {
            Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired)
        }
        ActiveComputeExecutionV1::Materialized(MaterializedCompletionReceiptV1::Consuming(
            MaterializedConsumeV1::Poll | MaterializedConsumeV1::Recycle,
        )) => None,
        #[cfg(test)]
        ActiveComputeExecutionV1::ScriptedMaterialized => {
            Some(RuntimeComputePipelinePhaseV1::Published)
        }
        #[cfg(test)]
        ActiveComputeExecutionV1::ScriptedMaterializedCompleted => {
            Some(RuntimeComputePipelinePhaseV1::Completed)
        }
        #[cfg(test)]
        ActiveComputeExecutionV1::ScriptedMaterializedRetired => {
            Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired)
        }
        _ => None,
    }
}

fn extent(lane: usize, writeback: &WritebackV1) -> NativeDirtyExtentV1 {
    NativeDirtyExtentV1 {
        compute_lane: lane,
        data_index: writeback.data_index,
        allocation_offset: writeback.allocation_offset,
        data_offset: writeback.data_offset,
        byte_len: writeback.byte_len,
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn materialized_completion_selected_v1(&self) -> bool {
        match self.active.as_ref().and_then(|a| a.execution.as_ref()) {
            Some(ActiveComputeExecutionV1::Materialized(_)) => true,
            #[cfg(test)]
            Some(
                ActiveComputeExecutionV1::ScriptedMaterialized
                | ActiveComputeExecutionV1::ScriptedMaterializedCompleted
                | ActiveComputeExecutionV1::ScriptedMaterializedRetired,
            ) => self.scripted_materialized_completion.is_some(),
            _ => false,
        }
    }

    fn materialized_completion_custody_v1(&self, target: MaterializedCompletionTargetV1) -> bool {
        let Some(active) = target.owner(self) else {
            return false;
        };
        let Some(recipe) = active.ordinary_recipe.as_ref() else {
            return false;
        };
        let Some(kernel) = self.kernels.get(&active.kernel) else {
            return false;
        };
        let Some(module) = self.modules.get(&kernel.module) else {
            return false;
        };
        let Some(phase) = execution_phase(active.execution.as_ref()) else {
            return false;
        };
        let source_intact = match phase {
            RuntimeComputePipelinePhaseV1::Published => active.source_event.may_publish(),
            RuntimeComputePipelinePhaseV1::Completed => active.source_event.may_complete(),
            RuntimeComputePipelinePhaseV1::PhysicallyRetired => active.source_event.may_retire(),
            _ => false,
        };
        if !source_intact {
            return false;
        }
        let lane = self.selected_compute_lane;
        if active.id == 0
            || lane >= self.native_compute_lanes.len()
            || recipe.kernel != active.kernel
            || recipe.stream != active.stream
            || self.streams.get(&active.stream) != Some(&module.device)
            || self.stream_compute_lanes.get(&active.stream) != Some(&lane)
            || (lane != 0
                && self
                    .auxiliary_compute_lanes
                    .get(lane - 1)
                    .is_none_or(|a| a.owner_stream != Some(active.stream)))
            || self
                .stream_submission_tails
                .get(&active.stream)
                .is_none_or(|tail| *tail < active.id)
            || self.compute_completion_reservations == 0
            || self.submissions.contains_key(&active.id)
            || self.pending_compute.contains_key(&active.id)
            || self.active_sdma.contains_key(&active.id)
            || self.resident_data.is_some()
            || self.native_reconciliation_pins_lane_v1(lane)
            || self
                .compute_module_retain_counts
                .get(&kernel.module)
                .is_none_or(|count| *count == 0)
            || active.allocations.len() != active.resident_descriptors.len()
            || !materialized_cancellation::materialized_descriptor_projection_intact_v1(
                &recipe.bindings,
                &active.resident_descriptors,
                &self.allocations,
            )
            || !Self::materialized_writebacks_intact_v1(active)
        {
            return false;
        }
        if let MaterializedCompletionTargetV1::Pipeline(identity) = target
            && (self
                .compute_pipeline
                .entry_v1(identity)
                .is_none_or(|entry| entry.phase != phase)
                || self
                    .active
                    .as_ref()
                    .is_none_or(|a| a.stream != active.stream || a.id == active.id))
        {
            return false;
        }
        if active.deferred_ordered_predecessor_retain
            && active.ordered_predecessor.is_none_or(|id| {
                id == active.id
                    || self
                        .compute_dependency_retain_counts
                        .get(&id)
                        .is_none_or(|count| *count == 0)
            })
        {
            return false;
        }
        for descriptor in &active.resident_descriptors {
            let allocation = descriptor.allocation;
            if !active.allocations.contains(&allocation)
                || self.allocations[&allocation].device != module.device
            {
                return false;
            }
            let expected = RuntimeAllocationCustodyOwnerV1 {
                submission: active.id,
                stream: active.stream,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            };
            let Some(custody) = self.allocation_custody.get(&allocation) else {
                return false;
            };
            let Ok(index) = custody
                .owners
                .binary_search_by_key(&active.id, |owner| owner.submission)
            else {
                return false;
            };
            if custody.owner_counts[RuntimeAllocationCustodyKindV1::Compute.index()] == 0
                || custody.owners[index] != expected
                || index
                    .checked_sub(1)
                    .and_then(|i| custody.owners.get(i))
                    .is_some_and(|owner| owner.submission == active.id)
                || custody
                    .owners
                    .get(index + 1)
                    .is_some_and(|owner| owner.submission == active.id)
            {
                return false;
            }
        }
        true
    }

    pub(super) fn advance_materialized_completion_v1(
        &mut self,
        target: MaterializedCompletionTargetV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !self.materialized_completion_custody_v1(target) {
            return Err(self.terminal_error("ordinary completion lost exact indexed custody"));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let scripted = false;
            #[cfg(test)]
            let scripted = scripted
                || matches!(
                    target.owner(self).unwrap().execution,
                    Some(
                        ActiveComputeExecutionV1::ScriptedMaterialized
                            | ActiveComputeExecutionV1::ScriptedMaterializedCompleted
                            | ActiveComputeExecutionV1::ScriptedMaterializedRetired
                    )
                );
            let retired = execution_phase(target.owner(self).unwrap().execution.as_ref())
                == Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired);
            if !scripted && !retired {
                self.observe_native_materialized_v1(target)?;
            }
            #[cfg(test)]
            if scripted && !retired {
                self.observe_scripted_materialized_v1(target)?;
            }
            let Some(phase) = execution_phase(target.owner(self).unwrap().execution.as_ref())
            else {
                return Err(self.terminal_error("ordinary completion returned invalid phase"));
            };
            if matches!(target, MaterializedCompletionTargetV1::Pipeline(_)) {
                return Ok(BackendPollV1::Pending);
            }
            if phase != RuntimeComputePipelinePhaseV1::PhysicallyRetired {
                return Ok(BackendPollV1::Pending);
            }
            self.commit_materialized_frontiers_v1()?;
            Ok(BackendPollV1::Succeeded)
        }));
        match result {
            Ok(result) => result,
            Err(payload) => {
                sdma_host_write::resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1())
            }
        }
    }

    // Returning the original linear receipt must not allocate on recycle retry.
    #[allow(clippy::result_large_err)]
    fn observe_native_materialized_v1(
        &mut self,
        target: MaterializedCompletionTargetV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let native_lane = self
            .selected_native_compute_lane_v1()
            .map_err(|_| self.terminal_error("ordinary completion lost native lane"))?;
        if !self.ordinary_queue_available_v1() {
            return Err(self.terminal_error("ordinary completion lost native queue"));
        }
        for operation in [MaterializedConsumeV1::Poll, MaterializedConsumeV1::Recycle] {
            if operation == MaterializedConsumeV1::Poll
                && execution_phase(target.owner(self).unwrap().execution.as_ref())
                    == Some(RuntimeComputePipelinePhaseV1::Completed)
            {
                continue;
            }
            let recycle_started = (operation == MaterializedConsumeV1::Recycle).then(Instant::now);
            if operation == MaterializedConsumeV1::Recycle
                && execution_phase(target.owner(self).unwrap().execution.as_ref())
                    == Some(RuntimeComputePipelinePhaseV1::Completed)
            {
                let (active, _) = target.parts_mut(&mut self.active, &mut self.compute_pipeline);
                let released = OrdinaryQueueIoV1::new(
                    self.queue.as_mut(),
                    #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
                    self.cpu_queue.as_deref_mut(),
                )
                .and_then(|queue| active.source_event.release_unused(queue));
                match released {
                    Ok(()) => {}
                    Err(error) => {
                        return Err(
                            self.terminal_error(format!("ordinary source event release: {error}"))
                        );
                    }
                }
            }
            let (active, phase) = target.parts_mut(&mut self.active, &mut self.compute_pipeline);
            let result = OrdinaryQueueIoV1::new(
                self.queue.as_mut(),
                #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
                self.cpu_queue.as_deref_mut(),
            )
            .and_then(|queue| {
                queue.with_lane(native_lane, |queue| {
                    let Some(ActiveComputeExecutionV1::Materialized(receipt)) =
                        active.execution.as_mut()
                    else {
                        unreachable!("preflighted ordinary completion receipt")
                    };
                    if operation == MaterializedConsumeV1::Poll
                        && matches!(receipt, MaterializedCompletionReceiptV1::Published(_))
                    {
                        if receipt.poll_ready(|batch| queue.poll(batch))? {
                            if let Some(phase) = phase {
                                *phase = RuntimeComputePipelinePhaseV1::Completed;
                            }
                            active.performance.publish_to_completion =
                                active.published_at.elapsed();
                        }
                    } else if operation == MaterializedConsumeV1::Recycle
                        && matches!(receipt, MaterializedCompletionReceiptV1::Completed(_))
                        && receipt.recycle_retired(|completed| queue.recycle(completed))?
                    {
                        active.performance.completed_readback = Duration::ZERO;
                        active.performance.completion_detach_restore = Duration::ZERO;
                        if let Some(phase) = phase {
                            *phase = RuntimeComputePipelinePhaseV1::PhysicallyRetired;
                        }
                    }
                    Ok::<(), fe2o3_kfd::ComputeAqlQueueSessionErrorV1>(())
                })
            });
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) | Err(error) => {
                    return Err(
                        self.terminal_error(format!("ordinary indexed completion: {error}"))
                    );
                }
            }
            if let Some(started) = recycle_started {
                let (active, _) = target.parts_mut(&mut self.active, &mut self.compute_pipeline);
                active.performance.completion_signal_recycle += started.elapsed();
            }
            if execution_phase(target.owner(self).unwrap().execution.as_ref())
                != Some(RuntimeComputePipelinePhaseV1::Completed)
            {
                break;
            }
        }
        Ok(())
    }

    fn materialized_commit_preflight_v1(&self) -> bool {
        let Some(active) = self.active.as_ref() else {
            return false;
        };
        if !self
            .materialized_completion_custody_v1(MaterializedCompletionTargetV1::Frontier(active.id))
            || execution_phase(active.execution.as_ref())
                != Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired)
            || self
                .compute_completion_reservations
                .checked_add(self.sdma_completion_reservations)
                .is_none_or(|reserved| {
                    self.submissions
                        .capacity()
                        .saturating_sub(self.submissions.len())
                        < reserved
                })
        {
            return false;
        }
        match self.compute_pipeline.checked_frontier_v1() {
            Err(()) => return false,
            Ok(Some(next))
                if next.active.stream != active.stream
                    || next.active.ordered_predecessor != Some(active.id)
                    || !next.active.deferred_ordered_predecessor_retain
                    || self
                        .compute_dependency_retain_counts
                        .get(&active.id)
                        .is_none_or(|count| *count == 0)
                    || next.active.ordinary_recipe != active.ordinary_recipe
                    || next.active.dispatch_shape_sha256 != active.dispatch_shape_sha256
                    || execution_phase(next.active.execution.as_ref()) != Some(next.phase) =>
            {
                return false;
            }
            _ => {}
        }
        let mut total = 0usize;
        for descriptor in &active.resident_descriptors {
            let record = &self.allocations[&descriptor.allocation];
            let mut added = 0usize;
            for (index, writeback) in active.writebacks.iter().enumerate() {
                if writeback.allocation != descriptor.allocation {
                    continue;
                }
                let wanted = extent(self.selected_compute_lane, writeback);
                if !record.native_dirty.contains(&wanted)
                    && !active.writebacks[..index].iter().any(|prior| {
                        prior.allocation == writeback.allocation
                            && extent(self.selected_compute_lane, prior) == wanted
                    })
                {
                    added += 1;
                }
            }
            if record.native_dirty.capacity() - record.native_dirty.len() < added {
                return false;
            }
            total += added;
        }
        self.native_dirty_extents.checked_add(total).is_some()
    }

    fn commit_materialized_frontiers_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        loop {
            if !self.materialized_commit_preflight_v1() {
                return Err(
                    self.terminal_error("ordinary completion cannot commit exact retired custody")
                );
            }
            let active = self.active.as_mut().unwrap();
            let (id, stream, kernel, depth, performance) = (
                active.id,
                active.stream,
                active.kernel,
                active.dependency_depth,
                active.performance,
            );
            let predecessor = active
                .deferred_ordered_predecessor_retain
                .then_some(active.ordered_predecessor)
                .flatten();
            for writeback in &active.writebacks {
                let record = self.allocations.get_mut(&writeback.allocation).unwrap();
                record.content_sha256 = None;
                if retain_unique_native_dirty_extent_v1(
                    &mut record.native_dirty,
                    extent(self.selected_compute_lane, writeback),
                ) {
                    self.native_dirty_extents += 1;
                }
                let descriptor = &mut active.resident_descriptors[writeback.data_index];
                descriptor.device_may_have_modified = true;
                descriptor.host_content_sha256 = None;
            }
            // No fallible operations or callbacks remain until logical settlement
            // and the next frontier are stable. Keep Active indexed throughout.
            let descriptor_count = active.resident_descriptors.len();
            for index in 0..descriptor_count {
                let allocation =
                    self.active.as_ref().unwrap().resident_descriptors[index].allocation;
                self.release_allocation_custody_v1(allocation, id);
            }
            self.release_compute_module_retain_v1(self.kernels[&kernel].module);
            if let Some(predecessor) = predecessor {
                self.release_compute_dependency_retains_v1(&[predecessor]);
            }
            let active = self.active.as_mut().unwrap();
            self.recycled_dispatch = Some(RecycledDispatchV1 {
                kernel,
                dispatch_shape_sha256: active.dispatch_shape_sha256,
                descriptors: core::mem::take(&mut active.resident_descriptors),
            });
            self.submissions.insert(
                id,
                SubmissionRecordV1 {
                    stream,
                    status: BackendPollV1::Succeeded,
                    dependency_depth: depth,
                    profile_dispatch_published: true,
                },
            );
            self.compute_completion_reservations -= 1;
            self.last_launch_performance = Some(performance);
            self.active = None;
            if let Some((_, next)) = self.compute_pipeline.take_commit_frontier() {
                self.active = Some(next);
            } else {
                self.release_compute_lane_lease_v1(stream, self.selected_compute_lane);
            }
            #[cfg(test)]
            self.scripted_materialized_profile_step_v1(id);
            let dispatch = self.profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id);
            self.observe_profile_v1(dispatch.map(|dispatch| {
                KfdRuntimeProfileEventKindV1::DispatchCompleted {
                    dispatch,
                    host_timing: profile_host_timing_v1(performance),
                }
            }));
            if self.active.as_ref().is_none_or(|a| {
                execution_phase(a.execution.as_ref())
                    != Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired)
            }) {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
#[path = "materialized_completion_script.rs"]
mod scripted;
#[cfg(test)]
pub(super) use scripted::ScriptedCompletionStepV1;

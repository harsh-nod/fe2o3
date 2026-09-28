//! Submit-only successors retain their own slot without replacing the frontier.

use super::materialized_publication::MaterializedSubmissionAttemptV1 as Attempt;
use super::*;

pub(super) struct OrderedPublicationV1 {
    pub(super) profile: PersistentPublicationProfileV1,
    pub(super) attempt: Attempt,
}

impl OrderedPublicationV1 {
    pub(super) fn new(profile: PersistentPublicationProfileV1) -> Self {
        Self {
            profile,
            attempt: Attempt::Unattempted,
        }
    }

    fn indexed(entry: &mut RuntimeComputePipelineEntryV1) -> &mut Self {
        let Some(ActiveComputeExecutionV1::MaterializedSuccessorPublication(root)) =
            entry.active.execution.as_mut()
        else {
            unreachable!("staged successor retains its publication root")
        };
        root
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn ordered_predecessor_execution_matches_v1(
        &self,
        active: &ActiveSubmissionV1,
        phase: Option<RuntimeComputePipelinePhaseV1>,
    ) -> bool {
        use RuntimeComputePipelinePhaseV1 as Phase;
        match (phase, active.execution.as_ref()) {
            (
                None | Some(Phase::Published),
                Some(ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Published(_),
                )),
            )
            | (
                None | Some(Phase::Completed),
                Some(ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Completed(_),
                )),
            ) => true,
            (
                Some(Phase::PhysicallyRetired),
                Some(ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Retired(observation),
                )),
            ) => observation.packet_count() == 1,
            #[cfg(test)]
            (
                None | Some(Phase::Published),
                Some(ActiveComputeExecutionV1::ScriptedMaterialized),
            )
            | (
                None | Some(Phase::Completed),
                Some(ActiveComputeExecutionV1::ScriptedMaterializedCompleted),
            ) => self.scripted_ordered_publication.is_some(),
            #[cfg(test)]
            (
                Some(Phase::PhysicallyRetired),
                Some(ActiveComputeExecutionV1::ScriptedMaterializedRetired),
            ) => {
                self.scripted_ordered_publication.is_some()
                    || self.scripted_materialized_completion.is_some()
            }
            _ => false,
        }
    }

    fn ordered_publication_custody_v1(
        &self,
        pending: &PendingComputeSubmissionV1,
        active: &ActiveSubmissionV1,
    ) -> bool {
        let lane = self.selected_compute_lane;
        let Some(recipe) = active.ordinary_recipe.as_ref() else {
            return false;
        };
        let Some(kernel) = self.kernels.get(&active.kernel) else {
            return false;
        };
        let Some(module) = self.modules.get(&kernel.module) else {
            return false;
        };
        if active.id == 0
            || active.id != pending.id
            || !Arc::ptr_eq(recipe, &pending.launch)
            || active.stream != recipe.stream
            || active.kernel != recipe.kernel
            || kernel.module != pending.module
            || pending.ordered_predecessor != active.ordered_predecessor
            || !active.deferred_ordered_predecessor_retain
            || active.ordered_predecessor.is_none_or(|id| {
                id == active.id
                    || self
                        .compute_dependency_retain_counts
                        .get(&id)
                        .is_none_or(|count| *count == 0)
            })
            || pending.explicit_dependency_cursor != pending.explicit_success_dependencies.len()
            || !pending.quiescence_dependencies.is_empty()
            || pending.explicit_success_dependencies.iter().any(|id| {
                self.compute_dependency_retain_counts
                    .get(id)
                    .is_none_or(|count| *count == 0)
            })
            || self
                .pending_compute_streams
                .get(&active.stream)
                .and_then(|fifo| fifo.front())
                != Some(&active.id)
            || lane >= self.native_compute_lanes.len()
            || self.stream_compute_lanes.get(&active.stream) != Some(&lane)
            || (lane != 0
                && self
                    .auxiliary_compute_lanes
                    .get(lane - 1)
                    .is_none_or(|aux| aux.owner_stream != Some(active.stream)))
            || self.streams.get(&active.stream) != Some(&module.device)
            || self
                .stream_submission_tails
                .get(&active.stream)
                .is_none_or(|tail| *tail < active.id)
            || self
                .active
                .as_ref()
                .is_none_or(|a| a.stream != active.stream || a.id == active.id)
            || self.pending_compute.contains_key(&active.id)
            || self.submissions.contains_key(&active.id)
            || self.active_sdma.contains_key(&active.id)
            || self.compute_pipeline.contains(active.id)
            || self.compute_completion_reservations == 0
            || self
                .compute_module_retain_counts
                .get(&kernel.module)
                .is_none_or(|count| *count == 0)
            || self.resident_data.is_some()
            || self.native_reconciliation_pins_lane_v1(lane)
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
        for descriptor in &active.resident_descriptors {
            let allocation = descriptor.allocation;
            if !active.allocations.contains(&allocation)
                || self.allocations[&allocation].device != module.device
            {
                return false;
            }
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
                || custody.owners[index]
                    != (RuntimeAllocationCustodyOwnerV1 {
                        submission: active.id,
                        stream: active.stream,
                        kind: RuntimeAllocationCustodyKindV1::Compute,
                    })
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

    pub(super) fn publish_indexed_ordered_successor_v1(
        &mut self,
        pending: &PendingComputeSubmissionV1,
        active: ActiveSubmissionV1,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let scripted = false;
        #[cfg(test)]
        let scripted = scripted || self.scripted_ordered_publication.is_some();
        if !scripted {
            self.selected_native_compute_lane_v1().map_err(|_| self.terminal_error(
                "ordered predecessor lost its exact physical compute lane before successor publication",
            ))?;
            if !self.ordinary_queue_available_v1() {
                return Err(self.terminal_error("ordered successor lost its native queue"));
            }
        }
        if !self.ordered_publication_custody_v1(pending, &active) {
            return Err(self.terminal_error("ordered successor lost exact accepted custody"));
        }
        let identity = match self.compute_pipeline.stage_publication_v1(active) {
            Ok(identity) => identity,
            Err(_) => return Ok(false),
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let started = Instant::now();
            if !scripted {
                self.submit_native_ordered_v1(identity)?;
            }
            #[cfg(test)]
            if scripted {
                self.submit_scripted_ordered_v1(identity)?;
            }
            self.compute_pipeline
                .entry_mut_v1(identity)
                .unwrap()
                .active
                .performance
                .publication = started.elapsed();
            self.finish_ordered_publication_v1(identity)
        }));
        match result {
            Ok(result) => result,
            Err(payload) => {
                sdma_host_write::resume_sdma_owner_panic_v1(payload, || self.poison_terminal_v1())
            }
        }
    }

    fn submit_native_ordered_v1(
        &mut self,
        identity: RuntimeComputePipelineIdentityV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let lane = self
            .selected_native_compute_lane_v1()
            .map_err(|_| self.terminal_error("staged successor lost its native lane"))?;
        let root =
            OrderedPublicationV1::indexed(self.compute_pipeline.entry_mut_v1(identity).unwrap());
        let result = OrdinaryQueueIoV1::new(
            self.queue.as_mut(),
            #[cfg(all(test, feature = "cpu-runtime-fixtures"))]
            self.cpu_queue.as_mut(),
        )
        .and_then(|queue| {
            queue.with_lane(lane, |queue| {
                root.attempt.submit_classified(|| queue.submit_classified())
            })
        });
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                Err(self.terminal_error(format!("ordered successor submission: {}", error.error())))
            }
            Err(error) => {
                Err(self.terminal_error(format!("ordered successor lane operation: {error}")))
            }
        }
    }

    fn finish_ordered_publication_v1(
        &mut self,
        identity: RuntimeComputePipelineIdentityV1,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root =
            OrderedPublicationV1::indexed(self.compute_pipeline.entry_mut_v1(identity).unwrap());
        if matches!(root.attempt, Attempt::Retryable) {
            return self
                .compute_pipeline
                .withdraw_publication_v1(identity)
                .map(|_| false)
                .ok_or_else(|| self.terminal_error("ordered retry lost its staged identity"));
        }
        if matches!(root.attempt, Attempt::Unattempted | Attempt::NativeOwned) {
            return Err(self.terminal_error("ordered publication has no confirmed outcome"));
        }
        self.compute_pipeline
            .entry_mut_v1(identity)
            .unwrap()
            .active
            .published_at = Instant::now();
        self.compute_pipeline
            .confirm_publication_v1(identity)
            .map_err(|()| self.terminal_error("ordered publication lost its staged identity"))?;
        let active = &mut self.compute_pipeline.entry_mut_v1(identity).unwrap().active;
        let Some(ActiveComputeExecutionV1::MaterializedSuccessorPublication(root)) =
            active.execution.take()
        else {
            unreachable!()
        };
        active.execution = Some(match root.attempt {
            Attempt::Published(batch) => ActiveComputeExecutionV1::Materialized(
                MaterializedCompletionReceiptV1::Published(batch),
            ),
            #[cfg(test)]
            Attempt::ScriptedPublished => ActiveComputeExecutionV1::ScriptedMaterialized,
            _ => unreachable!("confirmed publication outcome"),
        });
        let (id, stream, kernel, shape) = (
            active.id,
            active.stream,
            active.kernel,
            active.dispatch_shape_sha256,
        );
        #[cfg(test)]
        if self
            .scripted_ordered_publication
            .as_mut()
            .is_some_and(|steps| {
                if steps.front() == Some(&(id, ScriptedOrderedPublicationV1::ProfileUnwind)) {
                    steps.pop_front();
                    true
                } else {
                    false
                }
            })
        {
            panic!("scripted ordered profile unwind");
        }
        self.observe_materialized_dispatch_published_v1(id, stream, kernel, shape, root.profile);
        Ok(true)
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScriptedOrderedPublicationV1 {
    Publish,
    Retry,
    Reject,
    Terminal,
    SubmitUnwind,
    BeforeAttemptError,
    BeforeAttemptUnwind,
    OuterErrorRetry,
    OuterUnwindRetry,
    OuterErrorPublish,
    OuterUnwindPublish,
    ProfileUnwind,
}

#[cfg(test)]
impl KfdRuntimeBackendV1 {
    fn submit_scripted_ordered_v1(
        &mut self,
        identity: RuntimeComputePipelineIdentityV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        use ScriptedOrderedPublicationV1 as Step;
        let id = self.compute_pipeline.entry_v1(identity).unwrap().active.id;
        let Some((observed, step)) = self
            .scripted_ordered_publication
            .as_mut()
            .unwrap()
            .pop_front()
        else {
            return Err(self.terminal_error("missing ordered submission script"));
        };
        if observed != id || step == Step::ProfileUnwind {
            return Err(self.terminal_error("wrong ordered submission script"));
        }
        match step {
            Step::BeforeAttemptError => {
                return Err(self.terminal_error("scripted ordered lane error"));
            }
            Step::BeforeAttemptUnwind => panic!("scripted ordered lane unwind"),
            _ => {}
        }
        let root =
            OrderedPublicationV1::indexed(self.compute_pipeline.entry_mut_v1(identity).unwrap());
        root.attempt
            .submit(|| match step {
                Step::Reject | Step::Terminal => Err("scripted ordered submission failure"),
                Step::SubmitUnwind => panic!("scripted ordered submit unwind"),
                Step::Retry | Step::OuterErrorRetry | Step::OuterUnwindRetry => {
                    Ok(Attempt::Retryable)
                }
                _ => Ok(Attempt::ScriptedPublished),
            })
            .map_err(|error| self.terminal_error(error))?;
        match step {
            Step::OuterErrorRetry | Step::OuterErrorPublish => {
                Err(self.terminal_error("scripted ordered outer error"))
            }
            Step::OuterUnwindRetry | Step::OuterUnwindPublish => {
                panic!("scripted ordered outer unwind")
            }
            _ => Ok(()),
        }
    }
}

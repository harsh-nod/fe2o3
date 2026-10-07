use super::*;

impl KfdRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn poll_compute_lane_v1(
        &mut self,
        lane: usize,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.with_compute_lane_state_v1(lane, |backend| {
            if backend.materialized_prepared_selected_v1() {
                return backend.poll_materialized_prepared_v1();
            }
            if backend.persistent_prepared_selected_v1() {
                return backend.poll_persistent_prepared_v1();
            }
            #[cfg(test)]
            if backend.scripted_persistent_poll_pending_observations != 0
                && backend.active.as_ref().is_some_and(|active| {
                    matches!(
                        active.execution.as_ref(),
                        Some(ActiveComputeExecutionV1::ScriptedPersistent { .. })
                            | Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { .. })
                    )
                })
            {
                backend.scripted_persistent_poll_pending_observations -= 1;
                return Ok(BackendPollV1::Pending);
            }
            if backend.scalar_completion_selected_v1() {
                return backend.advance_scalar_completion_v1(None);
            }
            if backend.materialized_completion_selected_v1() {
                let id = backend.active.as_ref().unwrap().id;
                return backend.advance_materialized_completion_v1(
                    super::materialized_completion::MaterializedCompletionTargetV1::Frontier(id),
                );
            }
            #[cfg(test)]
            if backend.scripted_persistent_transition_failure
                == Some(ScriptedPersistentTransitionFailureV1::UnwindBeforeTake)
                && backend.active.as_ref().is_some_and(|active| {
                    matches!(
                        active.execution.as_ref(),
                        Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { .. })
                    )
                })
            {
                backend.scripted_persistent_transition_failure = None;
                panic!("scripted three-binding unwind before active take");
            }
            if backend.three_completion_selected_v1() {
                return backend.advance_three_completion_v1(None);
            }
            let Some(mut active) = backend.active.take() else {
                return Err(backend
                    .terminal_error("selected KFD compute lane lost its logical frontier owner"));
            };
            let Some(execution) = active.execution.take() else {
                backend.active = Some(active);
                return Err(
                    backend.terminal_error("active KFD submission lost its execution custody")
                );
            };
            match execution {
                execution @ (ActiveComputeExecutionV1::MaterializedPrepared(_)
                | ActiveComputeExecutionV1::MaterializedSuccessorPublication(_)) => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend.terminal_error("materialized retry bypassed its indexed path"))
                }
                execution @ ActiveComputeExecutionV1::Materialized(_) => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend.terminal_error("ordinary completion bypassed its indexed path"))
                }
                #[cfg(test)]
                execution @ (ActiveComputeExecutionV1::ScriptedMaterializedCompleted
                | ActiveComputeExecutionV1::ScriptedMaterializedRetired) => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend
                        .terminal_error("scripted ordinary completion bypassed its indexed path"))
                }
                execution @ ActiveComputeExecutionV1::PersistentPrepared { .. } => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend.terminal_error("prepared publication bypassed its indexed path"))
                }
                execution @ (ActiveComputeExecutionV1::Persistent { .. }
                | ActiveComputeExecutionV1::PersistentCompleting(_)) => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend.terminal_error("scalar completion bypassed its indexed path"))
                }
                execution @ ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { .. } => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend.terminal_error("prepared publication bypassed its indexed path"))
                }
                execution @ (ActiveComputeExecutionV1::ThreeBindingPersistent { .. }
                | ActiveComputeExecutionV1::ThreeBindingPersistentCompleting(_)) => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend
                        .terminal_error("three-binding completion bypassed its indexed path"))
                }
                #[cfg(test)]
                execution @ ActiveComputeExecutionV1::ScriptedPersistent { .. } => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend
                        .terminal_error("scripted scalar completion bypassed its indexed path"))
                }
                #[cfg(test)]
                execution @ ActiveComputeExecutionV1::ScriptedThreeBindingPersistent { .. } => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend.terminal_error(
                        "scripted three-binding completion bypassed its indexed path",
                    ))
                }
                #[cfg(test)]
                execution @ (ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. }
                | ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                    ..
                }) => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend
                        .terminal_error("scripted prepared publication bypassed its indexed path"))
                }
                ActiveComputeExecutionV1::PersistentCancelling(cancellation) => {
                    active.execution =
                        Some(ActiveComputeExecutionV1::PersistentCancelling(cancellation));
                    backend.active = Some(active);
                    Err(backend.terminal_error("prepared cancellation cannot resume publication"))
                }
                ActiveComputeExecutionV1::MaterializedCancelling(cancellation) => {
                    active.execution = Some(ActiveComputeExecutionV1::MaterializedCancelling(
                        cancellation,
                    ));
                    backend.active = Some(active);
                    Err(backend
                        .terminal_error("materialized cancellation cannot reenter publication"))
                }
                execution @ ActiveComputeExecutionV1::MaterializedBinding(_) => {
                    active.execution = Some(execution);
                    backend.active = Some(active);
                    Err(backend
                        .terminal_error("incomplete ordinary binding cannot resume publication"))
                }
                #[cfg(test)]
                ActiveComputeExecutionV1::ScriptedMaterialized => {
                    active.performance.publish_to_completion = active.published_at.elapsed();
                    backend.finish_scripted_materialized_compute_v1(active)
                }
            }
        })
    }

    pub(in crate::kfd_backend) fn poll_compute_submission_v1(
        &mut self,
        lane: usize,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let is_frontier = if lane == 0 {
            self.active
                .as_ref()
                .is_some_and(|active| active.id == submission)
        } else {
            self.auxiliary_compute_lanes[lane - 1]
                .active
                .as_ref()
                .is_some_and(|active| active.id == submission)
        };
        if is_frontier {
            return self.poll_compute_lane_v1(lane);
        }
        let status = self.with_compute_lane_state_v1(lane, |backend| {
            backend.poll_pipelined_compute_submission_v1(submission)
        })?;
        if status == BackendPollV1::Pending
            && self
                .active_compute_lane_v1(submission)
                .is_some_and(|owner| owner == lane)
        {
            // A waiter on a recycled successor must also own bounded progress
            // of the earlier logical frontier; otherwise the successor could
            // remain Pending forever after its physical retirement.
            let _ = self.poll_compute_lane_v1(lane)?;
            if let Some(record) = self.submissions.get(&submission) {
                return Ok(record.status);
            }
        }
        Ok(status)
    }

    pub(super) fn poll_pipelined_compute_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(identity) = self.compute_pipeline.identity_for_submission_v1(submission) else {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown KFD pipelined submission",
            ));
        };
        self.advance_materialized_completion_v1(
            super::materialized_completion::MaterializedCompletionTargetV1::Pipeline(identity),
        )
    }

    pub(in crate::kfd_backend) fn wait_published_persistent_compute_lane_v1(
        &mut self,
        lane: usize,
        deadline: Instant,
    ) -> Result<Option<BackendPollV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.with_compute_lane_state_v1(lane, |backend| {
            if backend.scalar_completion_selected_v1() {
                return backend
                    .advance_scalar_completion_v1(Some(deadline))
                    .map(Some);
            }
            if backend.three_completion_selected_v1() {
                return backend
                    .advance_three_completion_v1(Some(deadline))
                    .map(Some);
            }
            Ok(None)
        })
    }

    pub(super) fn poll_retained_pending_dependency_v1(
        &mut self,
        pending: PendingComputeSubmissionV1,
        dependency: u64,
    ) -> (
        PendingComputeSubmissionV1,
        Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    ) {
        // Dependency observation may unwind while this node is outside its
        // index. Keep its sole recipe and rosters outside the unwinding frame.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.poll_v1(dependency))) {
            Ok(result) => (pending, result),
            Err(payload) => {
                self.pending_compute.insert(pending.id, pending);
                super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                    self.poison_terminal_v1()
                })
            }
        }
    }

    pub(super) fn settle_failed_compute_after_ordering_v1(
        &mut self,
        mut pending: PendingComputeSubmissionV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        // A failed unpublished node still orders its successor after the entire
        // stream prefix. Keep its custody until that predecessor has completed.
        if let Some(predecessor) = pending.ordered_predecessor
            && !self.exact_submission_quiescent_v1(predecessor)
        {
            let (retained, result) = self.poll_retained_pending_dependency_v1(pending, predecessor);
            pending = retained;
            match result {
                Ok(BackendPollV1::Pending) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Ok(BackendPollV1::Pending);
                }
                Ok(BackendPollV1::Succeeded | BackendPollV1::Failed { .. })
                | Err(RuntimeBackendFailureV1::Quiescent(_)) => {}
                Err(failure) => {
                    self.pending_compute.insert(pending.id, pending);
                    return Err(failure);
                }
            }
            if !self.exact_submission_quiescent_v1(predecessor) {
                self.pending_compute.insert(pending.id, pending);
                return Ok(BackendPollV1::Pending);
            }
        }
        self.settle_failed_unpublished_compute_v1(pending, -1)
    }
}

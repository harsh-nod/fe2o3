//! Shared bounded reconciliation for explicitly success-gated backend profiles.

use super::peer_custody::ScalarPeerDependencyV1;
use super::*;
use fe2o3_runtime_model::ContextProducerReadStatusV1;

include!("completion_reconciliation_body.rs");

#[derive(Clone, Copy, Debug)]
pub(super) struct DirectedPeerStateV1 {
    pub(super) depth: usize,
    pub(super) cursor: usize,
    pub(super) terminal: Option<BackendPollV1>,
}

// The planner must observe an ordinary producer, never traverse its dependencies
// or infer completion from the success of its consumer.
const ORDINARY_PEER_OBSERVATION_LEAF_V1: DirectedPeerStateV1 = DirectedPeerStateV1 {
    depth: 1,
    cursor: 0,
    terminal: None,
};

enum CompletionStepV1 {
    Observe {
        id: RuntimeSubmissionIdV1,
        backend: u64,
    },
    Local(RuntimeCompletionStatusV1),
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    fn producer_completion_parts_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Option<(&[ScalarPeerDependencyV1], &DirectedPeerStateV1)> {
        if let Some(root) = self.same_device_copies.get(&id) {
            Some((&root.dependencies, &root.state))
        } else if let Some(root) = self.producer_launches.get(&id) {
            Some((&root.dependencies, &root.state))
        } else {
            self.scalar_peer_copies
                .get(&id)
                .map(|root| match root.directed.as_ref() {
                    Some(state) => (root.dependencies.as_slice(), state),
                    None => (&[][..], &ORDINARY_PEER_OBSERVATION_LEAF_V1),
                })
        }
    }

    fn producer_completion_state_mut_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Option<&mut DirectedPeerStateV1> {
        if let Some(root) = self.same_device_copies.get_mut(&id) {
            Some(&mut root.state)
        } else if let Some(root) = self.producer_launches.get_mut(&id) {
            Some(&mut root.state)
        } else {
            self.scalar_peer_copies
                .get_mut(&id)
                .and_then(|root| root.directed.as_mut())
        }
    }

    fn invalid_directed_v1<T>(&mut self) -> Result<T, RuntimeValidationErrorV1> {
        self.quarantine_after_async_command_panic_v1();
        Err(RuntimeValidationErrorV1::InvalidBackendDescription)
    }

    pub(super) fn retained_directed_success_v1(&self, id: RuntimeSubmissionIdV1) -> bool {
        self.producer_completion_parts_v1(id)
            .is_some_and(|(_, state)| state.terminal == Some(BackendPollV1::Succeeded))
    }

    pub(super) fn retain_directed_observation_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        observation: BackendPollV1,
    ) -> Result<bool, RuntimeValidationErrorV1> {
        self.check_operation_custody_v1(id)?;
        let Some(state) = self.producer_completion_state_mut_v1(id) else {
            return Ok(false);
        };
        if let Some(prior) = state.terminal {
            if prior != observation {
                return self.invalid_directed_v1();
            }
        } else if observation != BackendPollV1::Pending {
            // The original terminal device fact survives any later settlement failure.
            state.terminal = Some(observation);
        }
        Ok(true)
    }

    pub(super) fn require_directed_success_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.check_operation_custody_v1(id)?;
        if self
            .scalar_peer_copies
            .get(&id)
            .is_some_and(|root| root.directed.is_none())
        {
            return Ok(());
        }
        let Some((dependencies, state)) = self.producer_completion_parts_v1(id) else {
            return Ok(());
        };
        if state.terminal != Some(BackendPollV1::Succeeded)
            || state.cursor != dependencies.len()
            || dependencies.iter().any(|dep| {
                self.submissions
                    .get(&dep.submission)
                    .is_none_or(|record| record.status != RuntimeCompletionStatusV1::Succeeded)
            })
        {
            return self.invalid_directed_v1();
        }
        if !matches!(
            self.directed_input_status_v1(id)?,
            None | Some(ContextProducerReadStatusV1::Success)
        ) {
            return self.invalid_directed_v1();
        }
        Ok(())
    }

    fn plan_completion_step_v1(
        &mut self,
        requested: RuntimeSubmissionIdV1,
    ) -> Result<CompletionStepV1, RuntimeValidationErrorV1> {
        if self
            .submissions
            .get(&requested)
            .is_some_and(|record| record.same_device_copy && !record.status.is_terminal())
        {
            let result = self.validate_pending_same_device_copy_roots_v1(requested);
            self.journal_result_v1(result)?;
        }
        completion_reconciliation_body!(completion_settlement_rust_expr, self, requested, [])
    }

    #[cfg(test)]
    pub(super) fn reconcile_directed_counted_for_test_v1(
        &mut self,
        requested: RuntimeSubmissionIdV1,
    ) -> (
        Result<RuntimeCompletionStatusV1, RuntimeValidationErrorV1>,
        usize,
    ) {
        fn counted<B: RuntimeBackendV1>(
            context: &mut RuntimeContextV1<B>,
            requested: RuntimeSubmissionIdV1,
            steps: &mut usize,
        ) -> Result<CompletionStepV1, RuntimeValidationErrorV1> {
            completion_reconciliation_body!(
                completion_settlement_rust_expr, context, requested, [*steps += 1;]
            )
        }
        let mut steps = 0;
        let result = counted(self, requested, &mut steps);
        (result.map(|_| self.submissions[&requested].status), steps)
    }

    pub(super) fn reconcile_directed_success_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeValidationErrorV1> {
        self.plan_completion_step_v1(id)?;
        Ok(self.submissions[&id].status)
    }

    pub(super) fn observe_completion_step_v1(
        &mut self,
        requested: RuntimeSubmissionIdV1,
        observe: impl FnOnce(&mut B, u64) -> Result<BackendPollV1, RuntimeBackendFailureV1<B::Error>>,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeErrorV1<B::Error>> {
        let (id, backend) = match self.plan_completion_step_v1(requested)? {
            CompletionStepV1::Local(status) => return Ok(status),
            CompletionStepV1::Observe { id, backend } => (id, backend),
        };
        let result = self.invoke_journal_backend_v1(|backend_ref| {
            if id == requested {
                observe(backend_ref, backend)
            } else {
                backend_ref.poll_v1(backend)
            }
        });
        if id != requested {
            // A retained parent success requires every exact retained native producer to
            // be successful. A discarded result is conservative Unknown, not success.
            match &result {
                Ok(BackendPollV1::Pending | BackendPollV1::Failed { .. }) => {
                    if let Ok(observation) = result {
                        self.retain_directed_observation_v1(id, observation)?;
                    }
                    return self.invalid_directed_v1().map_err(Into::into);
                }
                Err(RuntimeBackendFailureV1::Rejected(_)) => {
                    self.quarantine_after_async_command_panic_v1()
                }
                _ => {}
            }
        }
        match self.completion_backend_result(id, result) {
            Ok(_) => {
                self.plan_completion_step_v1(requested)?;
                Ok(self.submissions[&requested].status)
            }
            Err(error) => {
                if !self.terminal {
                    // Preserve the backend diagnostic even if local settlement also fails.
                    if self.plan_completion_step_v1(requested).is_err() {
                        self.quarantine_after_async_command_panic_v1();
                    }
                }
                Err(error)
            }
        }
    }
}

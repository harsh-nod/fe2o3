//! Bounded logical reconciliation after a directed backend reports success.

use super::*;
use fe2o3_runtime_model::ContextProducerReadStatusV1;

#[derive(Clone, Copy, Debug)]
pub(super) struct DirectedPeerStateV1 {
    pub(super) depth: usize,
    pub(super) cursor: usize,
    pub(super) terminal: Option<BackendPollV1>,
}

enum CompletionStepV1 {
    Observe {
        id: RuntimeSubmissionIdV1,
        backend: u64,
    },
    Local(RuntimeCompletionStatusV1),
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    fn invalid_directed_v1<T>(&mut self) -> Result<T, RuntimeValidationErrorV1> {
        self.quarantine_after_async_command_panic_v1();
        Err(RuntimeValidationErrorV1::InvalidBackendDescription)
    }

    pub(super) fn retained_directed_success_v1(&self, id: RuntimeSubmissionIdV1) -> bool {
        self.scalar_peer_copies
            .get(&id)
            .and_then(|root| root.directed.as_ref())
            .is_some_and(|state| state.terminal == Some(BackendPollV1::Succeeded))
    }

    pub(super) fn retain_directed_observation_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        observation: BackendPollV1,
    ) -> Result<bool, RuntimeValidationErrorV1> {
        self.check_scalar_peer_custody_v1(id)?;
        let Some(state) = self
            .scalar_peer_copies
            .get_mut(&id)
            .and_then(|root| root.directed.as_mut())
        else {
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
        let Some(root) = self
            .scalar_peer_copies
            .get(&id)
            .filter(|root| root.directed.is_some())
        else {
            return Ok(());
        };
        let state = root.directed.as_ref().expect("directed root");
        if state.terminal != Some(BackendPollV1::Succeeded)
            || state.cursor != root.dependencies.len()
            || root.dependencies.iter().any(|dep| {
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
        let mut path = [requested; MAX_RUNTIME_DEPENDENCIES_V1];
        let mut length = 0;
        let mut id = requested;
        let mut validated = None;
        // Cursor advances survive a yield. Shared/diamond predecessors settle only once.
        for _ in 0..(2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1) {
            let record = *self
                .submissions
                .get(&id)
                .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
            if validated != Some(id) {
                self.require_ordinary_submission_v1(id)?;
                self.check_scalar_peer_custody_v1(id)?;
                if !record.status.is_terminal() && record.directed_peer_copy {
                    let result = self.validate_pending_peer_copy_roots_v1(id);
                    self.journal_result_v1(result)?;
                }
                validated = Some(id);
            }
            if record.status.is_terminal() {
                if length == 0 {
                    return Ok(CompletionStepV1::Local(record.status));
                }
                length -= 1;
                id = path[length];
                validated = None;
                continue;
            }
            let Some(root) = self
                .scalar_peer_copies
                .get(&id)
                .filter(|root| root.directed.is_some())
            else {
                if length != 0 {
                    return self.invalid_directed_v1();
                }
                return Ok(CompletionStepV1::Observe {
                    id,
                    backend: record.backend_submission,
                });
            };
            let state = *root.directed.as_ref().expect("directed root");
            match state.terminal {
                None => {
                    return Ok(CompletionStepV1::Observe {
                        id,
                        backend: record.backend_submission,
                    });
                }
                Some(BackendPollV1::Succeeded) => {}
                _ => return self.invalid_directed_v1(),
            }
            if let Some(dependency) = root.dependencies.get(state.cursor).copied() {
                match self.submissions[&dependency.submission].status {
                    RuntimeCompletionStatusV1::Succeeded => {
                        self.scalar_peer_copies
                            .get_mut(&id)
                            .expect("retained root")
                            .directed
                            .as_mut()
                            .expect("directed root")
                            .cursor += 1;
                    }
                    RuntimeCompletionStatusV1::Pending => {
                        if length >= path.len() {
                            return self.invalid_directed_v1();
                        }
                        path[length] = id;
                        length += 1;
                        id = dependency.submission;
                        validated = None;
                    }
                    RuntimeCompletionStatusV1::QuiescentWithoutResult => {
                        self.transition_submission_status(
                            id,
                            RuntimeCompletionStatusV1::QuiescentWithoutResult,
                        )?;
                        validated = None;
                    }
                    RuntimeCompletionStatusV1::Failed(_) => return self.invalid_directed_v1(),
                }
                continue;
            }
            match self.directed_input_status_v1(id)? {
                None | Some(ContextProducerReadStatusV1::Success) => {
                    self.transition_submission_status(id, RuntimeCompletionStatusV1::Succeeded)?;
                }
                Some(ContextProducerReadStatusV1::Unknown) => {
                    self.transition_submission_status(
                        id,
                        RuntimeCompletionStatusV1::QuiescentWithoutResult,
                    )?;
                }
                _ => return self.invalid_directed_v1(),
            }
            validated = None;
        }
        Ok(CompletionStepV1::Local(self.submissions[&requested].status))
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

impl Witness {
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
                self.check_operation_custody_v1(id)?;
                if !record.status.is_terminal() && record.directed_peer_copy {
                    let result = self.validate_pending_peer_copy_roots_v1(id);
                    self.journal_result_v1(result)?;
                }
                if !record.status.is_terminal() && record.producer_launch {
                    let result = self.validate_pending_producer_launch_roots_v1(id);
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
            let Some((dependencies, state)) = self.producer_completion_parts_v1(id) else {
                if length != 0 {
                    return self.invalid_directed_v1();
                }
                return Ok(CompletionStepV1::Observe {
                    id,
                    backend: record.backend_submission,
                });
            };
            let state = *state;
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
            if let Some(dependency) = dependencies.get(state.cursor).copied() {
                match self.submissions[&dependency.submission].status {
                    RuntimeCompletionStatusV1::Succeeded => {
                        self.producer_completion_state_mut_v1(id)
                            .expect("retained success-gated state")
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
}

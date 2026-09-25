// One executable planner body; custody, journal and settlement effects stay in its adapters.
macro_rules! completion_reconciliation_body {
    ($syntax:ident, $context:ident, $requested:ident, [$($on_step:tt)*]) => {
        completion_reconciliation_body!(@annotated $syntax, $context, $requested,
            path, length, id, validated, remaining, record, dependencies, state, dependency,
            [], [$($on_step)*], [], [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $context:ident, $requested:ident,
     $path:ident, $length:ident, $id:ident, $validated:ident, $remaining:ident,
     $record:ident, $dependencies:ident, $state:ident, $dependency:ident,
     [$($invariants:tt)*], [$($on_step:tt)*], [$($after_validation:tt)*],
     [$($after_pop:tt)*], [$($after_advance:tt)*], [$($after_descend:tt)*],
     [$($after_settlement:tt)*], [$($before_dependency:tt)*], [$($before_input:tt)*]) => {
        $syntax!({
            let mut $path = [$requested; MAX_RUNTIME_DEPENDENCIES_V1];
            let mut $length = 0usize;
            let mut $id = $requested;
            let mut $validated = None;
            let mut $remaining = 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1;
            // Cursor advances survive a yield. Shared/diamond predecessors settle only once.
            while $remaining > 0
                $($invariants)*
            {
                $remaining -= 1;
                $($on_step)*
                let $record = *$context
                    .submissions
                    .get(&$id)
                    .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
                if $validated != Some($id) {
                    $context.require_ordinary_submission_v1($id)?;
                    $context.check_operation_custody_v1($id)?;
                    if !$record.status.is_terminal() && $record.directed_peer_copy {
                        let result = $context.validate_pending_peer_copy_roots_v1($id);
                        $context.journal_result_v1(result)?;
                    }
                    if !$record.status.is_terminal() && $record.producer_launch {
                        let result = $context.validate_pending_producer_launch_roots_v1($id);
                        $context.journal_result_v1(result)?;
                    }
                    $validated = Some($id);
                }
                $($after_validation)*
                if $record.status.is_terminal() {
                    if $length == 0 {
                        return Ok(CompletionStepV1::Local($record.status));
                    }
                    $length -= 1;
                    $id = $path[$length];
                    $validated = None;
                    $($after_pop)*
                    continue;
                }
                let Some(($dependencies, $state)) = $context.producer_completion_parts_v1($id) else {
                    if $length != 0 {
                        return $context.invalid_directed_v1();
                    }
                    return Ok(CompletionStepV1::Observe {
                        id: $id,
                        backend: $record.backend_submission,
                    });
                };
                let $state = *$state;
                match $state.terminal {
                    None => {
                        return Ok(CompletionStepV1::Observe {
                            id: $id,
                            backend: $record.backend_submission,
                        });
                    }
                    Some(BackendPollV1::Succeeded) => {}
                    _ => return $context.invalid_directed_v1(),
                }
                if let Some($dependency) = $dependencies.get($state.cursor) {
                    let $dependency = *$dependency;
                    $($before_dependency)*
                    match $context.submissions[&$dependency.submission].status {
                        RuntimeCompletionStatusV1::Succeeded => {
                            $context.producer_completion_state_mut_v1($id)
                                .expect("retained success-gated state")
                                .cursor += 1;
                            $($after_advance)*
                        }
                        RuntimeCompletionStatusV1::Pending => {
                            if $length >= $path.len() {
                                return $context.invalid_directed_v1();
                            }
                            $path[$length] = $id;
                            $length += 1;
                            $id = $dependency.submission;
                            $validated = None;
                            $($after_descend)*
                        }
                        RuntimeCompletionStatusV1::QuiescentWithoutResult => {
                            let result = $context.transition_submission_status(
                                $id,
                                RuntimeCompletionStatusV1::QuiescentWithoutResult,
                            );
                            $($after_settlement)*
                            result?;
                            $validated = None;
                        }
                        RuntimeCompletionStatusV1::Failed(_) => return $context.invalid_directed_v1(),
                    }
                    continue;
                }
                $($before_input)*
                match $context.directed_input_status_v1($id)? {
                    None | Some(ContextProducerReadStatusV1::Success) => {
                        let result = $context.transition_submission_status($id, RuntimeCompletionStatusV1::Succeeded);
                        $($after_settlement)*
                        result?;
                    }
                    Some(ContextProducerReadStatusV1::Unknown) => {
                        let result = $context.transition_submission_status(
                            $id,
                            RuntimeCompletionStatusV1::QuiescentWithoutResult,
                        );
                        $($after_settlement)*
                        result?;
                    }
                    _ => return $context.invalid_directed_v1(),
                }
                $validated = None;
            }
            Ok(CompletionStepV1::Local($context.submissions[&$requested].status))
        })
    };
}

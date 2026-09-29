// The post-catch decision is shared; Rust unwind and native observations are separate boundaries.
macro_rules! retained_pair_operation_body {
    ($context:ident, $operation:ident) => {{
        let outcome = catch_unwind(AssertUnwindSafe(|| $operation($context)));
        match settle_operation($context, outcome) {
            Settled::Return(value) => value,
            Settled::Resume(payload) => resume_unwind(payload),
        }
    }};
}

macro_rules! retained_pair_terminal_body {
    ($pair:ident) => {{
        $pair.queue.require_live_queue_state_v1().is_err()
            || $pair.source.phase() != SharedMemorySessionPhaseV1::Active
            || $pair.destination.phase() != SharedMemorySessionPhaseV1::Active
    }};
}

macro_rules! retained_pair_settle_body {
    ($context:ident, $outcome:ident) => {{
        match $outcome {
            Ok(value) => {
                if $context.terminal() {
                    $context.quarantine();
                    Settled::Return(value.refuse_terminal_success())
                } else {
                    Settled::Return(value)
                }
            }
            Err(payload) => {
                $context.quarantine();
                Settled::Resume(payload)
            }
        }
    }};
}

macro_rules! retained_pair_unit_outcome_body {
    ($value:ident) => {{
        match $value {
            Ok(()) => Err(terminal_success_error()),
            failure => failure,
        }
    }};
}

macro_rules! retained_pair_tickets_outcome_body {
    ($value:ident) => {{
        match $value {
            Ok(tickets) => Err(Gfx942XgmiBatchSubmissionFailureV1::Retained {
                error: terminal_success_error(), tickets,
            }),
            failure => failure,
        }
    }};
}

macro_rules! retained_pair_completed_outcome_body {
    ($value:ident) => {{
        match $value {
            Ok(completed) => Err(Gfx942XgmiBatchWaitFailureV1::CompletedCurrentnessIndeterminate {
                error: terminal_success_error(), completed,
            }),
            failure => failure,
        }
    }};
}

macro_rules! retained_pair_finish_terminal_body {
    ($scope:ident) => {{
        $scope.context.quarantine();
        $scope.finished = true;
    }};
}

macro_rules! retained_pair_drop_body {
    ($scope:ident) => {{
        if !$scope.finished {
            $scope.context.quarantine();
        }
    }};
}

macro_rules! retained_pair_close_body {
    ($scope:ident, $close:ident) => {{
        let result = run_operation(&mut $scope.context, $close);
        retained_pair_close_post_body!($scope, result)
    }};
}

macro_rules! retained_pair_close_post_body {
    ($scope:ident, $result:ident) => {{
        if $result.is_err() {
            $scope.context.quarantine();
        }
        $scope.finished = true;
        $result
    }};
}

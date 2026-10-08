// Shared read-only cached-poll prefix. No backend or journal operation is present.
macro_rules! cached_poll_rust_expr {
    ($body:expr) => {
        $body
    };
}

macro_rules! cached_submission_record_body_v1 {
    ($syntax:ident, $context:ident, $submission:ident) => {
        $syntax!({
            if $submission.id.context_generation != $context.context_generation {
                return Err(RuntimeValidationErrorV1::UnknownSubmission);
            }
            let record = *$context
                .submissions
                .get(&$submission.id)
                .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
            if record.backend_submission != $submission.backend_submission
                || record.stream != $submission.stream
                || record.device != $submission.device
            {
                return Err(RuntimeValidationErrorV1::UnknownSubmission);
            }
            Ok(record)
        })
    };
}

macro_rules! cached_live_submission_body_v1 {
    ($syntax:ident, $context:ident, $submission:ident) => {
        $syntax!({
            let record = $context.submission_record($submission)?;
            let stream = $context
                .streams
                .get(&record.stream)
                .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
            if stream.device != record.device {
                return Err(RuntimeValidationErrorV1::WrongDevice);
            }
            Ok(record)
        })
    };
}

macro_rules! cached_unheld_stream_body_v1 {
    ($syntax:ident, $context:ident, $stream:ident) => {
        $syntax!({
            let record = $context
                .streams
                .get(&$stream)
                .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
            if record.unpublished.is_some() {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            Ok(record)
        })
    };
}

macro_rules! cached_poll_prefix_body_v1 {
    ($syntax:ident, $context:ident, $submission:ident) => {
        $syntax!({
            let record = $context.live_submission_record($submission)?;
            $context.require_stream_unheld_v1(record.stream)?;
            if record.status.is_terminal() {
                return Ok(Some($submission.observe_status(record.status)));
            }
            Ok(None)
        })
    };
}

macro_rules! cached_status_terminal_body_v1 {
    ($syntax:ident, $status:ident) => {
        $syntax!({ !matches!($status, Self::Pending) })
    };
}

macro_rules! cached_status_legacy_body_v1 {
    ($syntax:ident, $status:ident) => {
        $syntax!({
            match $status {
                Self::Pending => RuntimePollV1::Pending,
                Self::Succeeded => RuntimePollV1::Succeeded,
                Self::Failed(RuntimeCompletionFailureV1::BackendCode(code)) => {
                    RuntimePollV1::Failed { code }
                }
                Self::Failed(RuntimeCompletionFailureV1::Cancelled) => RuntimePollV1::Failed {
                    code: RUNTIME_CANCELLED_CODE_V1,
                },
                Self::QuiescentWithoutResult => RuntimePollV1::Failed {
                    code: RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1,
                },
            }
        })
    };
}

macro_rules! cached_observe_status_body_v1 {
    ($syntax:ident, $submission:ident, $status:ident) => {
        $syntax!({
            let observation = $status.legacy_poll();
            if $status.is_terminal() {
                $submission.completion = Some(observation);
            }
            observation
        })
    };
}

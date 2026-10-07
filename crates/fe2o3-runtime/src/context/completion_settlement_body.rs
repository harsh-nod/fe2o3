include!("completion_journal_prefix_body.rs");

// Shared with Verus: an error stops the suffix without invoking a rollback.
macro_rules! completion_settlement_execution_body {
    ($syntax:ident, $context:ident, $submission:ident, $status:ident, $outcome:ident) => {
        $syntax!({
            completion_journal_prefix_body!($syntax, $context, $submission, $outcome, [], []);
            match $context.release_operation_dependencies_v1($submission) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
            $context.publish_submission_status_v1($submission, $status)
        })
    };
}

// Shared with Verus: an error stops the suffix without invoking a rollback.
macro_rules! completion_settlement_execution_body {
    ($syntax:ident, $context:ident, $submission:ident, $status:ident, $outcome:ident) => {
        $syntax!({
            match $context.release_submission_inputs_v1($submission) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
            match $context.settle_submission_writer_v1($submission, $outcome) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
            match $context.release_operation_dependencies_v1($submission) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
            $context.publish_submission_status_v1($submission, $status)
        })
    };
}

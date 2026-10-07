// The called Context methods retain their own quarantine and unwind handling.
macro_rules! completion_journal_prefix_body {
    ($syntax:ident, $context:ident, $submission:ident, $outcome:ident,
     [$($inputs_done:tt)*], [$($writer_done:tt)*]) => {
        $syntax!({
            match $context.release_submission_inputs_v1($submission) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
            $($inputs_done)*
            let result = $context.settle_submission_writer_v1($submission, $outcome);
            $($writer_done)*
            match result {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
        })
    };
}

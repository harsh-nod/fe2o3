 completion_writer_outcome_declaration {
    ($syntax:ident, $visibility:vis) => {
        $syntax! {
            #[derive(Clone, Copy)]
            $visibility enum SubmissionWriterOutcomeV1 {
                Success,
                NoEffect,
                Unknown,
            }
        }
    };
}

// Prevalidation and both unwind boundaries remain in the Context adapters.
 completion_input_release_body {
    ($syntax:ident, $context:ident, $id:ident, $producer:ident,
     [$($stable_done:tt)*], [$($producer_done:tt)*]) => {
        $syntax!({
            match $context.release_submission_readers_v1($id) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
            $($stable_done)*
            if !$producer {
                return Ok(());
            }
            let result = $context.release_validated_submission_producer_readers_v1($id);
            $($producer_done)*
            result
        })
    };
}

 completion_writer_effect_body {
    ($syntax:ident, $journal:expr, $writer:ident, $outcome:ident,
     $success:ident, $no_effect:ident, [$($storage:tt)*]) => {
        $syntax!({
            match $outcome {
                SubmissionWriterOutcomeV1::Success => ($journal).$success(
                    $writer, &ContextWriterSuccessEvidenceV1 { writer: $writer } $($storage)*),
                SubmissionWriterOutcomeV1::NoEffect => ($journal).$no_effect(
                    $writer, &ContextWriterNoEffectEvidenceV1 { writer: $writer } $($storage)*),
                SubmissionWriterOutcomeV1::Unknown => ($journal).mark_unknown($writer),
            }
        })
    };
}

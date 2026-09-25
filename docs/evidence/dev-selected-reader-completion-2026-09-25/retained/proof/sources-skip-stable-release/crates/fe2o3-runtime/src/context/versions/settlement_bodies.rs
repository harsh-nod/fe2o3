macro_rules! completion_writer_outcome_declaration {
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
macro_rules! completion_input_release_body {
    ($syntax:ident, $context:ident, $id:ident, $producer:ident,
     [$($stable_done:tt)*], [$($producer_done:tt)*]) => {
        $syntax!({
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

// Look up the roster, commit its journal effect, then retire its Context root/marker.
// The adapter owns prevalidation, absence handling and the unwind boundary.
macro_rules! completion_selected_reader_release_body {
    ($syntax:ident, $roots:expr, $records:expr, $journal:expr,
     $id:ident, $marker:ident, $release:ident, [$($after_effect:tt)*]) => {
        $syntax!({
            let root = ($roots).get(&$id).expect("validated reader root");
            let consumer = root.marker.expect("validated reader marker").first.consumer;
            match ($journal).$release(
                consumer, &root.references, &ContextReadQuiescenceEvidenceV1 { consumer },
            ) {
                Ok(()) => (),
                Err(error) => return Err(error),
            }
            $($after_effect)*
            ($roots).remove(&$id);
            if let Some(record) = ($records).get_mut(&$id) {
                record.$marker = None;
            }
            Ok(())
        })
    };
}

macro_rules! completion_writer_effect_body {
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

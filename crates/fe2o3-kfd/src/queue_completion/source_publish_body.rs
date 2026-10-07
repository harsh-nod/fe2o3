macro_rules! completion_source_publish_body {
    ($syntax:ident, $owner:expr, $retention:ident, $last:ident, $events:ident) => {
        completion_source_publish_body!(@annotated $syntax, $owner, $retention, $last, $events,
            batch, failure, [], [], [])
    };
    (@annotated $syntax:ident, $owner:expr, $retention:ident, $last:ident, $events:ident,
        $batch:ident, $failure:ident, [$($mark_error:tt)*], [$($bind_error:tt)*], [$($published:tt)*]) => {
        $syntax!({
            let $batch = ($owner)
                .mark_published_retaining($retention, $last)
                .map_err(|$failure| $($mark_error)* {
                    FixedDispatchSubmissionFailureV1::Terminal(
                        ComputeAqlQueueSessionErrorV1::Completion($failure.0),
                    )
                })?;
            $($published)*
            let $events = ($owner)
                .bind_dependency_event_batch_v1($events, &$batch)
                .map_err(|$failure| $($bind_error)* {
                    FixedDispatchSubmissionFailureV1::Terminal(
                        ComputeAqlQueueSessionErrorV1::Completion($failure.0),
                    )
                })?;
            Ok(($batch, $events))
        })
    };
}

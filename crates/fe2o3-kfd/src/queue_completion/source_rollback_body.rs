// Inline expansion preserves the caller's ownership and lexical drop scopes.
macro_rules! completion_source_release_call {
    ($owner:expr, $events:ident) => {
        ($owner).release_dependency_event_batch_v1($events)
    };
}

macro_rules! completion_source_cancel_call {
    ($owner:expr, $retention:ident) => {
        ($owner).cancel_bound($retention)
    };
}

macro_rules! completion_source_rollback_body {
    ($syntax:ident, $owner:expr, $events:ident, $retention:ident) => {
        completion_source_rollback_body!(@annotated $syntax, $owner, $events, $retention,
            completion_source_release_call, completion_source_cancel_call)
    };
    (@annotated $syntax:ident, $owner:expr, $events:ident, $retention:ident,
     $release:ident, $cancel:ident) => {
        $syntax!({
            if $release!($owner, $events).is_err() || $cancel!($owner, $retention).is_err() {
                Err(Gfx942CompletionErrorV1::StaleEventOccurrence)
            } else {
                Ok(())
            }
        })
    };
}

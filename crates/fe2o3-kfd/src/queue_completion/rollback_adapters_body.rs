macro_rules! completion_cancel_bound_body {
    ($syntax:ident, $owner:ident, $retention:ident) => {
        completion_cancel_bound_body!(@annotated $syntax, $owner, $retention, failure, [])
    };
    (@annotated $syntax:ident, $owner:ident, $retention:ident, $failure:ident,
     [$($contract:tt)*]) => {
        $syntax!({
            $owner.cancel_bound_retaining($retention)
                .map_err(|$failure| $($contract)* { $failure.0 })
        })
    };
}

macro_rules! completion_release_dependency_event_batch_body {
    ($syntax:ident, $owner:ident, $events:ident) => {
        $syntax!({ $owner.release_compute_event_batch($events) })
    };
}

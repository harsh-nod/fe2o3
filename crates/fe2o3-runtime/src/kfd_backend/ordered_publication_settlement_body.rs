// The projection is identity in Rust and selects the modeled Active payload in
// Verus. Clock expressions remain at their production evaluation sites.
macro_rules! ordered_publication_indexed_body {
    ($syntax:ident, $fields:ident, $entry:ident) => {
        $syntax!({
            let Some(ActiveComputeExecutionV1::MaterializedSuccessorPublication(root)) =
                $fields!($entry.active).execution.as_mut()
            else {
                unreachable!("staged successor retains its publication root")
            };
            root
        })
    };
}

macro_rules! ordered_publication_settle_body {
    ($syntax:ident, $fields:ident, $pipeline:ident, $identity:ident,
     $elapsed:expr, $now:expr) => {
        ordered_publication_settle_body!(@annotated $syntax, $fields, $pipeline, $identity,
            $elapsed, $now, entry, root, active, id, [], [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $fields:ident, $pipeline:ident, $identity:ident,
     $elapsed:expr, $now:expr, $entry:ident, $root:ident, $active:ident, $id:ident,
     [$($before:tt)*], [$($after_duration:tt)*], [$($before_retry:tt)*],
     [$($after_withdraw:tt)*], [$($after_time:tt)*],
     [$($after_confirm:tt)*], [$($after_deposit:tt)*]) => {
        $syntax!({
            $($before)*
            let $entry = match $pipeline.entry_mut_v1($identity) {
                Some(entry) => entry,
                None => return Err(OrderedPublicationSettlementErrorV1::MissingIdentity),
            };
            $fields!($entry.active).performance.publication = $elapsed;
            $($after_duration)*
            let $root = OrderedPublicationV1::indexed($entry);
            if matches!($root.attempt, Attempt::Retryable) {
                $($before_retry)*
                return match $pipeline.withdraw_publication_v1($identity) {
                    Some(_) => { $($after_withdraw)* Ok(None) },
                    None => Err(OrderedPublicationSettlementErrorV1::RetryStage),
                };
            }
            if matches!($root.attempt, Attempt::Unattempted | Attempt::NativeOwned) {
                return Err(OrderedPublicationSettlementErrorV1::NoOutcome);
            }
            let $entry = $pipeline.entry_mut_v1($identity).unwrap();
            $fields!($entry.active).published_at = $now;
            $($after_time)*
            if $pipeline.confirm_publication_v1($identity).is_err() {
                return Err(OrderedPublicationSettlementErrorV1::ConfirmationStage);
            }
            $($after_confirm)*
            let $entry = $pipeline.entry_mut_v1($identity).unwrap();
            let $id = $entry.active.id;
            let $active = &mut $fields!($entry.active);
            let Some(ActiveComputeExecutionV1::MaterializedSuccessorPublication($root)) =
                $active.execution.take()
            else {
                unreachable!()
            };
            $active.execution = Some(match $root.attempt {
                Attempt::Published(batch) => ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Published(batch),
                ),
                #[cfg(test)]
                Attempt::ScriptedPublished => ActiveComputeExecutionV1::ScriptedMaterialized,
                _ => unreachable!("confirmed publication outcome"),
            });
            let observation = OrderedPublicationObservationV1 {
                id: $id,
                stream: $active.stream,
                kernel: $active.kernel,
                shape: $active.dispatch_shape_sha256,
                profile: $root.profile,
            };
            $($after_deposit)*
            Ok(Some(observation))
        })
    };
}

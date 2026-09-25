// Both complete preflights precede either mutation. Proof hooks add no Rust work.
macro_rules! mixed_acquire_execution_body {
    ($syntax:ident, $contents:ident, $generation:expr, $consumer:ident,
     $stable_requests:ident, $stable_output:ident, $producer_requests:ident, $producer_output:ident,
     $stable_preflight:path, $stable_commit:path, $producer_preflight:path, $producer_commit:path,
     $value:ident, [$($setup:tt)*], [$($stable_passed:tt)*], [$($producer_passed:tt)*],
     [$($ready:tt)*], [$($stable_committed:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($setup)*
            if $consumer.context_generation != $generation {
                return Err(ContextVersionJournalErrorV1::ForeignContext);
            }
            if $consumer.local == 0 || $consumer.local == u64::MAX
                || !matches!($consumer.kind, ContextWriterKindV1::Submission)
            {
                return Err(ContextVersionJournalErrorV1::InvalidWriterId);
            }
            if $stable_requests.len() != $stable_output.len()
                || $producer_requests.len() != $producer_output.len()
            {
                return Err(ContextVersionJournalErrorV1::RosterCapacity);
            }
            let count = match $stable_requests.len().checked_add($producer_requests.len()) {
                Some(count) => count,
                None => return Err(ContextVersionJournalErrorV1::MemberCapacity),
            };
            if count > $contents.remaining_read_slots() {
                return Err(ContextVersionJournalErrorV1::MemberCapacity);
            }
            if !$stable_requests.is_empty() {
                match $stable_preflight(&$contents.stable, $consumer, $stable_requests, $stable_output) {
                    Ok($value) => { $($stable_passed)* },
                    Err(error) => return Err(error),
                }
            }
            if !$producer_requests.is_empty() {
                match $producer_preflight($contents, $consumer, $producer_requests, $producer_output) {
                    Ok($value) => { $($producer_passed)* },
                    Err(error) => return Err(error),
                }
            }
            $($ready)*
            if !$stable_requests.is_empty() {
                $stable_commit(&mut $contents.stable, $consumer, $stable_requests, $stable_output);
            }
            $($stable_committed)*
            if !$producer_requests.is_empty() {
                $producer_commit($contents, $consumer, $producer_requests, $producer_output);
            }
            $($finish)*
            Ok(())
        })
    };
}

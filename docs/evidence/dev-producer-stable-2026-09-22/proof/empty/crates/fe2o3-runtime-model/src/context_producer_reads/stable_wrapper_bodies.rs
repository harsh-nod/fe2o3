// Stable consumers retain their original header ordering and combined-budget check.
macro_rules! producer_stable_acquire_header_body {
    ($contents:ident, $generation:expr, $consumer:ident, $count:ident, $output:ident) => {{
        if $consumer.context_generation != $generation {
            return Err(ContextVersionJournalErrorV1::ForeignContext);
        }
        if $consumer.local == 0 || $consumer.local == u64::MAX {
            return Err(ContextVersionJournalErrorV1::InvalidWriterId);
        }
        if $count != $output.len() {
            return Err(ContextVersionJournalErrorV1::RosterCapacity);
        }
        if !producer_stable_output_vacant_exec_v1($output) {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        $contents.validate_read_capacity($count)
    }};
}

macro_rules! producer_stable_acquire_body {
    ($syntax:ident, $contents:ident, $consumer:ident, $requests:ident, $output:ident,
     $header:path, $value:ident, $result:ident, [$($setup:tt)*], [$($passed:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($setup)*
            match $header($contents, $consumer, $requests.len(), $output) {
                Ok($value) => { $($passed)* },
                Err(error) => return Err(error),
            }
            let $result = $contents.stable.acquire_reads($consumer, $requests, $output);
            $($finish)*
            $result
        })
    };
}

macro_rules! producer_stable_release_body {
    ($contents:ident, $consumer:ident, $references:ident, $evidence:ident,
     $release:ident, [$($capacity_arg:tt)*]) => {
        $contents.stable.$release($consumer, $references, $evidence $($capacity_arg)*)
    };
}

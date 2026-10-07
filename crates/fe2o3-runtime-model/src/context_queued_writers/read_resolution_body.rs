// Shared executable resolution; the Rust invocation supplies no proof work.
macro_rules! queued_read_resolution_body {
    ($syntax:ident, $owner:ident, $root:ident, $status:ident,
     $next:ident, $index:ident, $slot:ident, $entry:ident, $state:ident, $retained:ident,
     [$($setup:tt)*], [$($invariant:tt)*], [$($before_entry:tt)*],
     [$($after_entry:tt)*], [$($before_root:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($setup)*
            let mut $next = $root.read_head;
            let mut $index = 0usize;
            while $index < $root.read_count
                $($invariant)*
            {
                let $slot = $next.expect("validated read list");
                $($before_entry)*
                let $entry = $owner.queued_reads[$slot]
                    .as_mut()
                    .expect("validated read");
                $next = $entry.next;
                $entry.status = $status;
                if $status == ContextProducerReadStatusV1::Success {
                    let $state = $owner.inner
                        .lookup_allocation($entry.request.allocation.allocation)
                        .expect("settled allocation retained by read");
                    $entry.version = Some(($state.attempt_epoch, $state.content_lineage));
                }
                $entry.previous = None;
                $entry.next = None;
                $($after_entry)*
                $index += 1;
            }
            $($before_root)*
            let $retained = $owner.roots[$root.writer.slot]
                .as_mut()
                .expect("retained outer root");
            $retained.read_head = None;
            $retained.read_count = 0;
            $($finish)*
        })
    };
}

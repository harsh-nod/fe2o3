// Ordered guard execution shared by both owners; annotations add no runtime work.
 stable_reader_count_body {
    ($contents:ident, $allocation:ident) => {{
        match $contents.journal.lookup_allocation($allocation) {
            Ok(_) => {},
            Err(error) => return Err(error),
        }
        Ok($contents.readers[$allocation.slot])
    }};
}

 producer_reader_count_body {
    ($contents:ident, $allocation:ident) => {{
        let stable = match $contents.stable.reader_count($allocation) {
            Ok(count) => count,
            Err(error) => return Err(error),
        };
        Ok(stable + $contents.counts[$allocation.slot])
    }};
}

 unread_writes_body {
    ($syntax:ident, $contents:ident, $roster:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $roster.len()
                $($annotations)*
            {
                let count = match $contents.reader_count($roster[$index].allocation) {
                    Ok(count) => count,
                    Err(error) => return Err(error),
                };
                if count != 0 {
                    return Err(ContextVersionJournalErrorV1::AllocationBusy);
                }
                $index += 1;
            }
            Ok(())
        })
    };
}

 stable_begin_body {
    ($contents:ident, $writer:ident, $roster:ident) => {{
        match $contents.require_unread_writes($roster) {
            Ok(_) => {},
            Err(error) => return Err(error),
        }
        $contents.journal.begin_write($writer, $roster)
    }};
}

 producer_begin_body {
    ($contents:ident, $writer:ident, $roster:ident) => {{
        
        $contents.stable.begin_write($writer, $roster)
    }};
}

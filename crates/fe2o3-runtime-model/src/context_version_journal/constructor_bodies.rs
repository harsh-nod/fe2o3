// Reservation outcomes are observed at the call boundary; allocation and unwind
// behavior remain the allocator's responsibility.
macro_rules! constructor_entry_body {
    ($construct:path, $allocator:expr, $($argument:expr),+ $(,)?) => {
        $construct($($argument,)+ $allocator)
    };
}

macro_rules! constructor_try_body {
    ($syntax:ident, $value:expr, [$($failure:tt)*]) => {
        $syntax!({
            match $value {
                Ok(value) => value,
                Err(error) => { $($failure)* return Err(error); },
            }
        })
    };
}

macro_rules! constructor_reserve_body {
    ($allocator:ident, $site:ident, $capacity:ident, $storage:ident) => {
        if !$allocator.reserve($site, $capacity, &mut $storage) {
            return Err(ContextVersionJournalErrorV1::StorageAllocationFailed);
        }
    };
}

macro_rules! constructor_vacant_body {
    ($syntax:ident, $element:ty, $capacity:ident, $site:ident, $allocator:ident,
     $slots:ident, [$($before:tt)*], [$($annotations:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($before)*
            let mut $slots: Vec<Option<$element>> = Vec::new();
            constructor_reserve_body!($allocator, $site, $capacity, $slots);
            while $slots.len() < $capacity
                $($annotations)*
            {
                $slots.push(None);
            }
            $($finish)*
            Ok($slots)
        })
    };
}

macro_rules! constructor_free_body {
    ($syntax:ident, $capacity:ident, $site:ident, $allocator:ident,
     $free:ident, $next:ident, [$($before:tt)*], [$($annotations:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($before)*
            let mut $free: Vec<usize> = Vec::new();
            constructor_reserve_body!($allocator, $site, $capacity, $free);
            let mut $next = $capacity;
            while $next > 0
                $($annotations)*
            {
                $next -= 1;
                $free.push($next);
            }
            $($finish)*
            Ok($free)
        })
    };
}

macro_rules! constructor_zero_body {
    ($syntax:ident, $capacity:ident, $site:ident, $allocator:ident,
     $counts:ident, [$($before:tt)*], [$($annotations:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($before)*
            let mut $counts: Vec<usize> = Vec::new();
            constructor_reserve_body!($allocator, $site, $capacity, $counts);
            while $counts.len() < $capacity
                $($annotations)*
            {
                $counts.push(0);
            }
            $($finish)*
            Ok($counts)
        })
    };
}

macro_rules! constructor_journal_body {
    ($syntax:ident, $context:ident, $allocations:ident, $writers:ident, $allocator:ident,
     $vacant:path, $free:path, $result:ident, [$($before:tt)*], [$($failure:tt)*], [$($finish:tt)*], [$($extra_fields:tt)*]) => {
        $syntax!({
            $($before)*
            if $context == 0 || $context == u64::MAX {
                return Err(ContextVersionJournalErrorV1::InvalidContextGeneration);
            }
            if $allocations == 0 || $allocations > CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1
                || $writers == 0 || $writers > CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1
            {
                return Err(ContextVersionJournalErrorV1::InvalidCapacity);
            }
            let writers = constructor_try_body!($syntax, $vacant($writers, 0, $allocator), [$($failure)*]);
            let free = constructor_try_body!($syntax, $free($writers, 1, $allocator), [$($failure)*]);
            let allocations = constructor_try_body!($syntax, $vacant($allocations, 2, $allocator), [$($failure)*]);
            let allocation_free = constructor_try_body!($syntax, $free($allocations, 3, $allocator), [$($failure)*]);
            let members = constructor_try_body!($syntax, $vacant($allocations, 4, $allocator), [$($failure)*]);
            let member_free = constructor_try_body!($syntax, $free($allocations, 5, $allocator), [$($failure)*]);
            let scratch = constructor_try_body!($syntax, $vacant($allocations, 6, $allocator), [$($failure)*]);
            let $result = Self {
                context_generation: $context, allocation_capacity: $allocations, writer_capacity: $writers,
                registration_watermark: 0, reserved_count: 0,
                writers, free, allocations, allocation_free, members, member_free, scratch,
                $($extra_fields)*
            };
            $($finish)*
            Ok($result)
        })
    };
}

macro_rules! constructor_stable_body {
    ($syntax:ident, $context:ident, $allocations:ident, $writers:ident, $reads:ident, $allocator:ident,
     $journal:path, $vacant:path, $free:path, $zero:path, $result:ident,
     [$($before:tt)*], [$($failure:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($before)*
            if $reads == 0 || $reads > CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1 {
                return Err(ContextVersionJournalErrorV1::InvalidCapacity);
            }
            let journal = constructor_try_body!($syntax, $journal($context, $allocations, $writers, $allocator), [$($failure)*]);
            let leases = constructor_try_body!($syntax, $vacant($reads, 7, $allocator), [$($failure)*]);
            let free_reads = constructor_try_body!($syntax, $free($reads, 8, $allocator), [$($failure)*]);
            let readers = constructor_try_body!($syntax, $zero($allocations, 9, $allocator), [$($failure)*]);
            let $result = Self { journal, leases, free_reads, readers, next_incarnation: 1 };
            $($finish)*
            Ok($result)
        })
    };
}

macro_rules! constructor_producer_body {
    ($syntax:ident, $context:ident, $allocations:ident, $writers:ident, $reads:ident, $allocator:ident,
     $stable:path, $vacant:path, $free:path, $zero:path, $result:ident,
     [$($before:tt)*], [$($failure:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($before)*
            let stable = constructor_try_body!($syntax, $stable($context, $allocations, $writers, $reads, $allocator), [$($failure)*]);
            let reservations = constructor_try_body!($syntax, $vacant($reads, 10, $allocator), [$($failure)*]);
            let free = constructor_try_body!($syntax, $free($reads, 11, $allocator), [$($failure)*]);
            let counts = constructor_try_body!($syntax, $zero($allocations, 12, $allocator), [$($failure)*]);
            let $result = Self { stable, reservations, free, counts, next_incarnation: 1 };
            $($finish)*
            Ok($result)
        })
    };
}

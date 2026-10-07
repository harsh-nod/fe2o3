macro_rules! completion_roster_project {
    ($project:ident, $value:expr) => {
        $project($value)
    };
}

macro_rules! completion_roster_hash_length {
    ($value:expr, $hasher:expr) => {
        core::hash::Hash::hash(&$value, $hasher)
    };
}

macro_rules! completion_roster_hash_binding {
    ($value:expr, $hasher:expr) => {
        core::hash::Hash::hash(&$value, $hasher)
    };
}

// The first projection is cached. Refusal occurs before hashing the failing
// binding, preserving the original Rust Hash feed and reached-prefix behavior.
macro_rules! completion_hash_roster_body {
    ($syntax:ident, $values:ident, $project:ident, $hasher:ident) => {
        completion_hash_roster_body!(@annotated $syntax, $values, $project, $hasher,
            completion_roster_project, completion_roster_hash_length,
            completion_roster_hash_binding, first, index, dispatch,
            [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $values:ident, $project:ident, $hasher:ident,
     $projection:ident, $hash_length:ident, $hash_binding:ident,
     $first:ident, $index:ident, $dispatch:ident,
     [$($initial:tt)*], [$($first_ready:tt)*], [$($invariants:tt)*],
     [$($before:tt)*], [$($step:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            $($initial)*
            let $first = $values.first().ok_or(Gfx942CompletionErrorV1::ZeroPacketCount)?;
            let $first = $projection!($project, $first);
            if $first.dispatch_generation == 0 {
                return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
            }
            $hash_length!($values.len(), $hasher);
            $hash_binding!($first, $hasher);
            let mut $index = 1;
            $($first_ready)*
            while $index < $values.len() $($invariants)* {
                $($before)*
                let $dispatch = $projection!($project, &$values[$index]);
                if $dispatch.queue != $first.queue
                    || $dispatch.dispatch_generation != $first.dispatch_generation
                {
                    return Err(Gfx942CompletionErrorV1::StaleBatchGeneration);
                }
                $hash_binding!($dispatch, $hasher);
                $($step)*
                $index += 1;
            }
            $($finish)*
            Ok(($first.queue, $first.dispatch_generation))
        })
    };
}

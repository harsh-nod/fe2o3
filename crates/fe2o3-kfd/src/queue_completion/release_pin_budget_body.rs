// The single-item selector, fallible reservation and caller projections remain
// outside these shared iterator bodies.
macro_rules! completion_single_release_pin_budget_body {
    ($syntax:ident, $budgets:ident, $insufficient:ident) => {
        $syntax!({
            match $budgets.next() {
                Some((_, 0)) => Err($insufficient),
                _ => Ok(()),
            }
        })
    };
}

macro_rules! completion_reserved_release_pin_budgets_body {
    ($syntax:ident, $budgets:ident, $remaining:ident, $insufficient:ident) => {
        completion_reserved_release_pin_budgets_body!(@annotated $syntax, $budgets,
            $remaining, $insufficient, index, pins, available, [], [], [], [], [], [])
    };
    (@annotated $syntax:ident, $budgets:ident, $remaining:ident, $insufficient:ident,
     $index:ident, $pins:ident, $available:ident, [$($snapshot:tt)*], [$($invariants:tt)*],
     [$($before_next:tt)*], [$($after_next:tt)*], [$($after_entry:tt)*], [$($after_debit:tt)*]) => {
        $syntax!({
            $($snapshot)*
            loop $($invariants)* {
                $($before_next)*
                let Some(($index, $pins)) = $budgets.next() else {
                    return Ok(());
                };
                $($after_next)*
                let $available = $remaining.entry($index).or_insert($pins);
                $($after_entry)*
                let Some(next) = $available.checked_sub(1) else {
                    return Err($insufficient);
                };
                *$available = next;
                $($after_debit)*
            }
        })
    };
}

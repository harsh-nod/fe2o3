// Proof hooks contain only erased annotations; all executable decisions live here.
macro_rules! enrollment_less_body {
    ($left:ident, $right:ident) => {{
        $left.context_generation < $right.context_generation
            || ($left.context_generation == $right.context_generation && $left.local < $right.local)
    }};
}

#[allow(unused_macros)]
macro_rules! enrollment_swap_body {
    ($values:ident, $left:ident, $right:ident) => {{
        let a = $values[$left];
        let b = $values[$right];
        $values[$left] = b;
        $values[$right] = a;
    }};
}

#[allow(unused_macros)]
macro_rules! enrollment_sorted_body {
    ($syntax:ident, $values:ident, $index:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let mut $index = 1usize;
            while $index < $values.len()
                $($invariants)*
            {
                if $values[$index - 1].unwrap().slot > $values[$index].unwrap().slot {
                    return false;
                }
                $index += 1;
            }
            true
        })
    };
}

#[allow(unused_macros)]
macro_rules! enrollment_sift_body {
    ($syntax:ident, $values:ident, $start:ident, $end:ident,
     $root:ident, $left:ident, $child:ident,
     [$($initial:tt)*], [$($invariants:tt)*], [$($exit:tt)*],
     [$($before_swap:tt)*], [$($after_swap:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            let mut $root = $start;
            $($initial)*
            while $root < $end / 2
                $($invariants)*
            {
                let $left = $root * 2 + 1;
                let mut $child = $left;
                if $left + 1 < $end
                    && $values[$left].unwrap().slot > $values[$left + 1].unwrap().slot
                {
                    $child = $left + 1;
                }
                if $values[$root].unwrap().slot >= $values[$child].unwrap().slot {
                    $($exit)*
                    return;
                }
                $($before_swap)*
                enrollment_swap($values, $root, $child);
                $($after_swap)*
                $root = $child;
            }
            $($finish)*
        })
    };
}

#[allow(unused_macros)]
macro_rules! enrollment_heapsort_body {
    ($syntax:ident, $values:ident, $len:ident, $start:ident, $end:ident,
     [$($initial:tt)*], [$($build_invariants:tt)*], [$($sort_invariants:tt)*],
     [$($before_swap:tt)*], [$($after_swap:tt)*], [$($after_sift:tt)*]) => {
        $syntax!({
            let $len = $values.len();
            let mut $start = $len / 2;
            $($initial)*
            while $start > 0
                $($build_invariants)*
            {
                $start -= 1;
                enrollment_sift($values, $start, $len);
            }
            let mut $end = $len;
            while $end > 1
                $($sort_invariants)*
            {
                $end -= 1;
                $($before_swap)*
                enrollment_swap($values, 0, $end);
                $($after_swap)*
                enrollment_sift($values, 0, $end);
                $($after_sift)*
            }
        })
    };
}

#[allow(unused_macros)]
macro_rules! enrollment_sort_body {
    ($values:ident) => {{
        if !enrollment_sorted($values) {
            enrollment_heapsort($values);
        }
    }};
}

macro_rules! enrollment_contains_key_body {
    ($syntax:ident, $entries:ident, $key:ident, $lo:ident, $hi:ident, $mid:ident, $found:ident,
     [$($invariants:tt)*], [$($lower:tt)*], [$($upper:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            let mut $lo = 0usize;
            let mut $hi = $entries.len();
            while $lo < $hi
                $($invariants)*
            {
                let $mid = $lo + ($hi - $lo) / 2;
                if enrollment_less($entries[$mid].key, $key) {
                    $($lower)*
                    $lo = $mid + 1;
                } else {
                    $($upper)*
                    $hi = $mid;
                }
            }
            if $lo == $entries.len() {
                return false;
            }
            let entry = $entries[$lo];
            let $found = entry.key.context_generation == $key.context_generation
                && entry.key.local == $key.local;
            $($finish)*
            $found
        })
    };
}

macro_rules! enrollment_contains_slot_body {
    ($syntax:ident, $values:ident, $slot:ident, $lo:ident, $hi:ident, $mid:ident, $found:ident,
     [$($invariants:tt)*], [$($lower:tt)*], [$($upper:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            let mut $lo = 0usize;
            let mut $hi = $values.len();
            while $lo < $hi
                $($invariants)*
            {
                let $mid = $lo + ($hi - $lo) / 2;
                if $values[$mid].unwrap().slot < $slot {
                    $($lower)*
                    $lo = $mid + 1;
                } else {
                    $($upper)*
                    $hi = $mid;
                }
            }
            if $lo == $values.len() {
                return false;
            }
            let $found = $values[$lo].unwrap().slot == $slot;
            $($finish)*
            $found
        })
    };
}

// Shared production depth-limited sort. Annotation hooks erase from Rust.
 enrollment_ordered_body {
    ($syntax:ident, $values:ident, $reverse:ident, $index:ident, $previous:ident, [$($invariants:tt)*]) => {
        $syntax!({
            if $values.len() < 2 {
                return true;
            }
            let mut $previous = $values[0].unwrap().slot;
            let mut $index = 1usize;
            while $index < $values.len()
                $($invariants)*
            {
                let next = $values[$index].unwrap().slot;
                if (!$reverse && $previous > next) || ($reverse && $previous < next) {
                    return false;
                }
                $previous = next;
                $index += 1;
            }
            true
        })
    };
}

 enrollment_reverse_halves_body {
    ($syntax:ident, $left:ident, $right:ident, $index:ident, $len:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let $len = if $left.len() < $right.len() { $left.len() } else { $right.len() };
            let mut $index = 0usize;
            while $index < $len
                $($invariants)*
            {
                core::mem::swap(&mut $left[$index], &mut $right[$len - 1 - $index]);
                $index += 1;
            }
        })
    };
}

 enrollment_reverse_body {
    ($syntax:ident, $values:ident, $half:ident, $left:ident, $tail:ident, $middle:ident, $right:ident,
     [$($split:tt)*], [$($reversed:tt)*]) => {
        $syntax!({
            let $half = $values.len() / 2;
            let ($left, $tail) = $values.split_at_mut($half);
            let skip = $tail.len() - $half;
            let ($middle, $right) = $tail.split_at_mut(skip);
            $($split)*
            enrollment_reverse_halves($left, $right);
            $($reversed)*
        })
    };
}

 enrollment_insertion_body {
    ($syntax:ident, $values:ident, $index:ident, $hole:ident, $held:ident,
     [$($outer:tt)*], [$($initialized:tt)*], [$($inner:tt)*], [$($before_shift:tt)*], [$($after_shift:tt)*],
     [$($before_place:tt)*], [$($after_place:tt)*]) => {
        $syntax!({
            let mut $index = 1usize;
            while $index < $values.len()
                $($outer)*
            {
                let $held = $values[$index];
                let mut $hole = $index;
                $($initialized)*
                while $hole > 0 && $values[$hole - 1].unwrap().slot > $held.unwrap().slot
                    $($inner)*
                {
                    $($before_shift)*
                    $values[$hole] = $values[$hole - 1];
                    $hole -= 1;
                    $($after_shift)*
                }
                $($before_place)*
                if $hole != $index {
                    $values[$hole] = $held;
                }
                $($after_place)*
                $index += 1;
            }
        })
    };
}

 enrollment_median_body {
    ($first:ident, $middle:ident, $last:ident) => {{
        if $first > $middle {
            if $middle < $last {
                $middle
            } else if $first < $last {
                $last
            } else {
                $first
            }
        } else if $first < $last {
            $first
        } else if $middle < $last {
            $last
        } else {
            $middle
        }
    }};
}

 enrollment_pivot_body {
    ($values:ident) => {{
        let len = $values.len();
        let middle = len / 2;
        if len < 128 {
            enrollment_median_of_three($values[0].unwrap().slot, $values[middle].unwrap().slot,
                $values[len - 1].unwrap().slot)
        } else {
            let step = len / 8;
            let first = enrollment_median_of_three($values[0].unwrap().slot,
                $values[step].unwrap().slot, $values[step * 2].unwrap().slot);
            let center = enrollment_median_of_three($values[middle - step].unwrap().slot,
                $values[middle].unwrap().slot, $values[middle + step].unwrap().slot);
            let last = enrollment_median_of_three($values[len - 1 - step * 2].unwrap().slot,
                $values[len - 1 - step].unwrap().slot, $values[len - 1].unwrap().slot);
            enrollment_median_of_three(first, center, last)
        }
    }};
}

 enrollment_partition_body {
    ($syntax:ident, $values:ident, $pivot:ident, $less:ident, $scan:ident, $greater:ident,
     [$($invariants:tt)*]) => {
        $syntax!({
            let mut $less = 0usize;
            let mut $scan = 0usize;
            let mut $greater = $values.len();
            while $scan < $greater
                $($invariants)*
            {
                let slot = $values[$scan].unwrap().slot;
                if slot < $pivot {
                    enrollment_swap($values, $less, $scan);
                    $less += 1;
                    $scan += 1;
                } else if slot > $pivot {
                    $greater -= 1;
                    enrollment_swap($values, $scan, $greater);
                } else {
                    $scan += 1;
                }
            }
            ($less, $greater)
        })
    };
}

 enrollment_depth_body {
    ($syntax:ident, $len:ident, $remaining:ident, $depth:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let mut $remaining = $len;
            let mut $depth = 0u32;
            while $remaining > 1 && $depth < 64
                $($invariants)*
            {
                $remaining /= 2;
                $depth += 1;
            }
            $depth * 2
        })
    };
}

 enrollment_binary_partition_body {
    ($syntax:ident, $values:ident, $pivot:ident, $left:ident, $right:ident,
     [$($outer:tt)*], [$($iteration:tt)*], [$($forward:tt)*], [$($backward:tt)*]) => {
        $syntax!({
            let mut $left = 0usize;
            let mut $right = $values.len();
            while $left < $right
                $($outer)*
            {
                $($iteration)*
                while $left < $right && $values[$left].unwrap().slot < $pivot
                    $($forward)*
                {
                    $left += 1;
                }
                while $left < $right && $values[$right - 1].unwrap().slot > $pivot
                    $($backward)*
                {
                    $right -= 1;
                }
                if $left < $right {
                    $right -= 1;
                    enrollment_swap($values, $left, $right);
                    $left += 1;
                }
            }
            ($left, $left)
        })
    };
}

 enrollment_lomuto_body {
    ($syntax:ident, $values:ident, $pivot:ident, $less:ident, $scan:ident, $gap:ident,
     $held:ident, $below:ident, [$($initial:tt)*], [$($invariants:tt)*],
     [$($before_shift:tt)*], [$($after_shift:tt)*], [$($before_close:tt)*], [$($after_close:tt)*]) => {
        $syntax!({
            if $values.len() == 0 {
                return (0, 0);
            }
            let $held = $values[0];
            let mut $less = 0usize;
            let mut $gap = 0usize;
            let mut $scan = 1usize;
            $($initial)*
            // The held first value closes the moving gap after the cyclic pass.
            while $scan < $values.len()
                $($invariants)*
            {
                let $below = if $values[$scan].unwrap().slot < $pivot { 1usize } else { 0usize };
                $($before_shift)*
                $values[$gap] = $values[$less];
                $values[$less] = $values[$scan];
                $gap = $scan;
                $less += $below;
                $scan += 1;
                $($after_shift)*
            }
            let $below = if $held.unwrap().slot < $pivot { 1usize } else { 0usize };
            $($before_close)*
            $values[$gap] = $values[$less];
            $values[$less] = $held;
            $less += $below;
            $($after_close)*
            ($less, $less)
        })
    };
}

 enrollment_partition_adaptive_body {
    ($values:ident, $pivot:ident) => {{
        let first = $values[0].unwrap().slot;
        let middle = $values[$values.len() / 2].unwrap().slot;
        let last = $values[$values.len() - 1].unwrap().slot;
        if (first == $pivot && (middle == $pivot || last == $pivot))
            || (middle == $pivot && last == $pivot) {
            enrollment_partition($values, $pivot)
        } else if $values.len() >= 256 {
            enrollment_lomuto($values, $pivot)
        } else {
            enrollment_binary_partition($values, $pivot)
        }
    }};
}

 enrollment_introsort_body {
    ($syntax:ident, $values:ident, $depth:ident, $pivot:ident, $less:ident, $greater:ident,
     $left:ident, $tail:ident, $middle:ident, $right:ident,
     [$($partitioned:tt)*], [$($split:tt)*], [$($sorted:tt)*]) => {
        $syntax!({
            if $values.len() <= 20 {
                enrollment_insertion($values);
                return;
            }
            if $depth == 0 {
                enrollment_heapsort($values);
                return;
            }
            let $pivot = enrollment_pivot($values);
            let ($less, $greater) = enrollment_partition_adaptive($values, $pivot);
            $($partitioned)*
            let ($left, $tail) = $values.split_at_mut($less);
            let ($middle, $right) = $tail.split_at_mut($greater - $less);
            $($split)*
            enrollment_introsort($left, $depth - 1);
            enrollment_introsort($right, $depth - 1);
            $($sorted)*
        })
    };
}

 enrollment_adaptive_body {
    ($syntax:ident, $values:ident, [$($reversed:tt)*]) => {
        $syntax!({
            if $values.len() <= 16 {
                enrollment_insertion($values);
                return;
            }
            if enrollment_ordered($values, false) {
                return;
            }
            if enrollment_ordered($values, true) {
                enrollment_reverse($values);
                $($reversed)*
                return;
            }
            if $values.len() <= 20 {
                enrollment_insertion($values);
                return;
            }
            let depth = enrollment_depth($values.len());
            enrollment_introsort($values, depth);
        })
    };
}

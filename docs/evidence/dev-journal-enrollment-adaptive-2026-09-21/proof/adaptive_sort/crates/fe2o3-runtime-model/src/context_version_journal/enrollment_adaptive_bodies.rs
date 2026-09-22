// Shared depth-limited sorting candidate. Annotation hooks erase from Rust.
macro_rules! enrollment_ordered_body {
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

macro_rules! enrollment_reverse_body {
    ($syntax:ident, $values:ident, $index:ident, $len:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let $len = $values.len();
            let mut $index = 0usize;
            while $index < $len / 2
                $($invariants)*
            {
                enrollment_swap($values, $index, $len - 1 - $index);
                $index += 1;
            }
        })
    };
}

macro_rules! enrollment_insertion_body {
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

macro_rules! enrollment_pivot_body {
    ($values:ident) => {{
        let first = $values[0].unwrap().slot;
        let middle = $values[$values.len() / 2].unwrap().slot;
        let last = $values[$values.len() - 1].unwrap().slot;
        if first < middle {
            if middle < last {
                middle
            } else if first < last {
                last
            } else {
                first
            }
        } else if first < last {
            first
        } else if middle < last {
            last
        } else {
            middle
        }
    }};
}

macro_rules! enrollment_partition_body {
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

macro_rules! enrollment_depth_body {
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

macro_rules! enrollment_binary_partition_body {
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

macro_rules! enrollment_lomuto_body {
    ($syntax:ident, $values:ident, $pivot:ident, $less:ident, $scan:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let mut $less = 0usize;
            let mut $scan = 0usize;
            while $scan < $values.len()
                $($invariants)*
            {
                let below = if $values[$scan].unwrap().slot < $pivot { 1usize } else { 0usize };
                enrollment_swap($values, $less, $scan);
                $less += below;
                $scan += 1;
            }
            ($less, $less)
        })
    };
}

macro_rules! enrollment_partition_adaptive_body {
    ($values:ident, $pivot:ident) => {{
        let first = $values[0].unwrap().slot;
        let middle = $values[$values.len() / 2].unwrap().slot;
        let last = $values[$values.len() - 1].unwrap().slot;
        if first == middle || first == last || middle == last {
            enrollment_partition($values, $pivot)
        } else if $values.len() >= 256 {
            enrollment_lomuto($values, $pivot)
        } else {
            enrollment_binary_partition($values, $pivot)
        }
    }};
}

macro_rules! enrollment_introsort_body {
    ($syntax:ident, $values:ident, $depth:ident, $pivot:ident, $less:ident, $greater:ident,
     $left:ident, $tail:ident, $middle:ident, $right:ident,
     [$($partitioned:tt)*], [$($split:tt)*], [$($sorted:tt)*]) => {
        $syntax!({
            if $values.len() <= 16 {
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

macro_rules! enrollment_adaptive_body {
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
            let depth = enrollment_depth($values.len());
            let _ = depth;
        })
    };
}

// Shared arithmetic; annotation hooks contain only proof and loop metadata.
macro_rules! resource_vector_zero_body_v1 {
    () => { Self { counts: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] } };
}

macro_rules! resource_vector_reserve_body_v1 {
    ($syntax:ident, $used:ident, $charge:ident, $capacity:ident,
     $next:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $next = R67ResourceVectorV1::ZERO;
            let mut $index = 0usize;
            while $index < R67_RESOURCE_DIMENSIONS_V1
                $($annotations)*
            {
                let value = match $used.counts[$index].checked_add($charge.counts[$index]) {
                    Some(value) => value,
                    None => return None,
                };
                if value > $capacity.counts[$index] { return None; }
                $next.counts[$index] = value;
                $index += 1;
            }
            Some($next)
        })
    };
}

macro_rules! resource_vector_release_body_v1 {
    ($syntax:ident, $used:ident, $charge:ident,
     $next:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $next = R67ResourceVectorV1::ZERO;
            let mut $index = 0usize;
            while $index < R67_RESOURCE_DIMENSIONS_V1
                $($annotations)*
            {
                $next.counts[$index] = match $used.counts[$index].checked_sub($charge.counts[$index]) {
                    Some(value) => value,
                    None => return None,
                };
                $index += 1;
            }
            Some($next)
        })
    };
}

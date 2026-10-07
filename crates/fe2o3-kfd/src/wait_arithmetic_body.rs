// Shared numeric declarations and bodies; no clocks, Duration methods or closures.
macro_rules! monotonic_wait_arithmetic_declarations_v1 {
    ($items:ident) => {
        $items! {
            const SPIN_ATTEMPTS_V1: u32 = 64;
            const YIELD_ATTEMPTS_V1: u32 = 16;
            const WAIT_NANOS_PER_SECOND_V1: u32 = 1_000_000_000;

            enum WaitPrefixV1 {
                Spin,
                Yield,
                Sleep,
            }
        }
    };
}

macro_rules! monotonic_wait_arithmetic_body_v1 {
    (increment, $attempts:ident) => {{
        if $attempts == u32::MAX {
            u32::MAX
        } else {
            $attempts + 1
        }
    }};
    (prefix, $attempts:ident) => {{
        if $attempts <= SPIN_ATTEMPTS_V1 {
            WaitPrefixV1::Spin
        } else if $attempts <= SPIN_ATTEMPTS_V1 + YIELD_ATTEMPTS_V1 {
            WaitPrefixV1::Yield
        } else {
            WaitPrefixV1::Sleep
        }
    }};
    (sleep, $next:ident, $remaining:ident) => {{
        match $remaining {
            Some(remaining) => remaining.0 < $next.0
                || (remaining.0 == $next.0 && remaining.1 < $next.1),
            None => false,
        }
    }};
    (backoff, $next:ident, $ceiling:ident) => {{
        let doubled = if $next.0 > u64::MAX / 2 {
            (u64::MAX, WAIT_NANOS_PER_SECOND_V1 - 1)
        } else {
            let doubled_nanos = $next.1 * 2;
            let (carry, nanos) = if doubled_nanos >= WAIT_NANOS_PER_SECOND_V1 {
                (1_u64, doubled_nanos - WAIT_NANOS_PER_SECOND_V1)
            } else {
                (0_u64, doubled_nanos)
            };
            ($next.0 * 2 + carry, nanos)
        };
        if doubled.0 < $ceiling.0
            || (doubled.0 == $ceiling.0 && doubled.1 < $ceiling.1)
        {
            doubled
        } else {
            $ceiling
        }
    }};
}

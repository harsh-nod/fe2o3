include!("wait_arithmetic_body.rs");

macro_rules! wait_arithmetic_rust_items {
    ($($item:item)*) => { $($item)* };
}

monotonic_wait_arithmetic_declarations_v1!(wait_arithmetic_rust_items);

fn increment_wait_attempts_v1(attempts: u32) -> u32 {
    monotonic_wait_arithmetic_body_v1!(increment, attempts)
}

fn wait_prefix_v1(attempts: u32) -> WaitPrefixV1 {
    monotonic_wait_arithmetic_body_v1!(prefix, attempts)
}

fn wait_sleep_uses_remaining_v1(next: (u64, u32), remaining: Option<(u64, u32)>) -> bool {
    monotonic_wait_arithmetic_body_v1!(sleep, next, remaining)
}

fn wait_backoff_pair_v1(next: (u64, u32), ceiling: (u64, u32)) -> (u64, u32) {
    monotonic_wait_arithmetic_body_v1!(backoff, next, ceiling)
}

// The std getters, value selection and constructor are tested separately;
// their correspondence to canonical numeric pairs is not part of the proof.
fn wait_sleep_duration_v1(next: Duration, remaining: Option<Duration>) -> Duration {
    match remaining {
        Some(remaining) => {
            if wait_sleep_uses_remaining_v1(
                (next.as_secs(), next.subsec_nanos()),
                Some((remaining.as_secs(), remaining.subsec_nanos())),
            ) {
                remaining
            } else {
                next
            }
        }
        None => next,
    }
}

fn wait_backoff_duration_v1(next: Duration, ceiling: Duration) -> Duration {
    let (seconds, nanos) = wait_backoff_pair_v1(
        (next.as_secs(), next.subsec_nanos()),
        (ceiling.as_secs(), ceiling.subsec_nanos()),
    );
    Duration::new(seconds, nanos)
}

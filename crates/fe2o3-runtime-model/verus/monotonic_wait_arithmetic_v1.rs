// Draft of four shared numeric helpers, not the native std/Instant adapter,
// whole cursor, constructors, observation loop, scheduler or hardware.
use vstd::prelude::*;
include!("../../fe2o3-kfd/src/wait_arithmetic_body.rs");
monotonic_wait_arithmetic_declarations_v1!(verus);

verus! {

spec fn min_nat_v1(left: nat, right: nat) -> nat {
    if left < right { left } else { right }
}

spec fn canonical_duration_pair_v1(value: (u64, u32)) -> bool {
    value.1 < WAIT_NANOS_PER_SECOND_V1
}

spec fn duration_pair_ns_v1(value: (u64, u32)) -> nat {
    value.0 as nat * 1_000_000_000 + value.1 as nat
}

spec fn selected_sleep_pair_v1(
    next: (u64, u32), remaining: Option<(u64, u32)>, use_remaining: bool,
) -> (u64, u32) {
    match remaining {
        Some(value) => if use_remaining { value } else { next },
        None => next,
    }
}

fn increment_wait_attempts_v1(attempts: u32) -> (result: u32)
    ensures result as nat == if attempts == u32::MAX {
        u32::MAX as nat
    } else { attempts as nat + 1 },
{
    monotonic_wait_arithmetic_body_v1!(increment, attempts)
}

fn wait_prefix_v1(attempts: u32) -> (result: WaitPrefixV1)
    ensures match result {
        WaitPrefixV1::Spin => attempts <= 64,
        WaitPrefixV1::Yield => 64 < attempts && attempts <= 80,
        WaitPrefixV1::Sleep => 80 < attempts,
    },
{
    monotonic_wait_arithmetic_body_v1!(prefix, attempts)
}

fn wait_sleep_uses_remaining_v1(
    next: (u64, u32), remaining: Option<(u64, u32)>,
) -> (result: bool)
    requires
        canonical_duration_pair_v1(next),
        match remaining { Some(value) => canonical_duration_pair_v1(value), None => true },
    ensures
        match remaining {
            Some(value) => result == (duration_pair_ns_v1(value) < duration_pair_ns_v1(next)),
            None => !result,
        },
        canonical_duration_pair_v1(selected_sleep_pair_v1(next, remaining, result)),
        duration_pair_ns_v1(selected_sleep_pair_v1(next, remaining, result)) == match remaining {
            Some(value) => min_nat_v1(duration_pair_ns_v1(value), duration_pair_ns_v1(next)),
            None => duration_pair_ns_v1(next),
        },
{
    monotonic_wait_arithmetic_body_v1!(sleep, next, remaining)
}

fn wait_backoff_pair_v1(next: (u64, u32), ceiling: (u64, u32)) -> (result: (u64, u32))
    requires canonical_duration_pair_v1(next), canonical_duration_pair_v1(ceiling),
    ensures
        canonical_duration_pair_v1(result),
        duration_pair_ns_v1(result) <= duration_pair_ns_v1(ceiling),
        duration_pair_ns_v1(result) == min_nat_v1(
            min_nat_v1(duration_pair_ns_v1(next) * 2,
                u64::MAX as nat * 1_000_000_000 + 999_999_999),
            duration_pair_ns_v1(ceiling)),
{
    monotonic_wait_arithmetic_body_v1!(backoff, next, ceiling)
}

}

// Expected-negative R48 mutation: panic guard permits one retirement.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_panic_retired_count_v1() -> nat { 1 }
pub proof fn panic_guard_retires_nothing_v1()
    ensures mutated_panic_retired_count_v1() == 0,
{}
}

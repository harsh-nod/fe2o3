// Expected-negative R48 mutation: timeout retires one request.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_retired_count_v1() -> nat { 1 }
pub proof fn timeout_has_zero_partial_retirement_v1()
    ensures mutated_timeout_retired_count_v1() == 0,
{}
}

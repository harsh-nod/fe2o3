// Expected-negative R40 mutation: failed preflight retires a prefix.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_failed_preflight_retired_v1() -> nat { 1 }
pub proof fn mutated_failed_preflight_retires_nothing_v1()
    ensures mutated_failed_preflight_retired_v1() == 0,
{}
}

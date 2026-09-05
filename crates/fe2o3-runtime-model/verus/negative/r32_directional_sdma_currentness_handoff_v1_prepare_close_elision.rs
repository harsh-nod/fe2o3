// Expected-negative R32 mutation: a preparation failure skips the retained close.
use vstd::prelude::*;

verus! {
pub struct StateV1 { pub checks: nat, pub close_observed: bool }

pub open spec fn mutated_prepare_failure_v1() -> StateV1 {
    StateV1 { checks: 1, close_observed: false }
}

pub proof fn mutated_prepare_failure_retains_old_close_v1()
    ensures mutated_prepare_failure_v1().checks == 2,
        mutated_prepare_failure_v1().close_observed,
{}

}

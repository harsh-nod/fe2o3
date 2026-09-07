// Expected-negative R42 mutation: backwards-epoch cycle rejection changes state.
use vstd::prelude::*;
verus! {
pub open spec fn state_before_v1() -> nat { 41 }
pub open spec fn mutated_state_after_cycle_failure_v1() -> nat { 42 }
pub proof fn mutated_cycle_failure_has_no_mutation_v1()
    ensures mutated_state_after_cycle_failure_v1() == state_before_v1(),
{}
}
